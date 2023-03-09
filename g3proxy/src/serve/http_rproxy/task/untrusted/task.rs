/*
 * Copyright 2023 ByteDance and/or its affiliates.
 *
 * Licensed under the Apache License, Version 2.0 (the "License");
 * you may not use this file except in compliance with the License.
 * You may obtain a copy of the License at
 *
 *     http://www.apache.org/licenses/LICENSE-2.0
 *
 * Unless required by applicable law or agreed to in writing, software
 * distributed under the License is distributed on an "AS IS" BASIS,
 * WITHOUT WARRANTIES OR CONDITIONS OF ANY KIND, either express or implied.
 * See the License for the specific language governing permissions and
 * limitations under the License.
 */

use std::sync::Arc;

use log::debug;
use tokio::io::{AsyncRead, AsyncWrite};
use tokio::time::Instant;

use g3_http::server::HttpProxyClientRequest;
use g3_http::HttpBodyReader;
use g3_io_ext::{LimitedCopy, LimitedCopyError};

use super::protocol::{HttpClientReader, HttpClientWriter, HttpRProxyRequest};
use super::{CommonTaskContext, UntrustedCltReadWrapperStats};
use crate::config::server::ServerConfig;
use crate::module::http_forward::HttpProxyClientResponse;
use crate::serve::{ServerTaskError, ServerTaskResult};

pub(crate) struct HttpRProxyUntrustedTask<'a> {
    ctx: Arc<CommonTaskContext>,
    req: &'a HttpProxyClientRequest,
    should_close: bool,
}

impl<'a> HttpRProxyUntrustedTask<'a> {
    pub(crate) fn new(
        ctx: &Arc<CommonTaskContext>,
        req: &'a HttpRProxyRequest<impl AsyncRead>,
    ) -> Self {
        HttpRProxyUntrustedTask {
            ctx: Arc::clone(ctx),
            req: &req.inner,
            should_close: !req.inner.keep_alive(),
        }
    }

    fn pre_start(&self) {
        debug!(
            "HttpRProxy/UNTRUSTED: new client from {} to {} server {}, using escaper {}",
            self.ctx.tcp_client_addr,
            self.ctx.server_config.server_type(),
            self.ctx.server_config.name(),
            self.ctx.server_config.escaper
        );
        self.ctx.server_stats.task_http_untrusted.add_task();
        self.ctx.server_stats.task_http_untrusted.inc_alive_task();
    }

    fn pre_stop(&self) {
        self.ctx.server_stats.task_http_untrusted.dec_alive_task();
    }

    #[inline]
    pub(crate) fn should_close(&self) -> bool {
        self.should_close
    }

    pub(crate) async fn run<CDR, CDW>(
        &mut self,
        clt_r: &mut Option<HttpClientReader<CDR>>,
        clt_w: &mut HttpClientWriter<CDW>,
    ) where
        CDR: AsyncRead + Unpin,
        CDW: AsyncWrite + Unpin,
    {
        if self.req.body_type().is_none() {
            self.reply_auth_error(clt_w).await;
        } else if let Some(br) = clt_r {
            if self.req.has_auth_info() || self.ctx.server_config.untrusted_read_limit.is_none() {
                // untrusted read is not permitted, we should close the connection
                self.should_close = true;
            }

            self.reply_auth_error(clt_w).await;

            if !self.should_close {
                if let Some(limit_config) = &self.ctx.server_config.untrusted_read_limit {
                    self.pre_start();

                    br.reset_limit(limit_config.shift_millis, limit_config.max_north);
                    let buffer_stats =
                        UntrustedCltReadWrapperStats::new_obj(&self.ctx.server_stats);
                    br.reset_buffer_stats(buffer_stats);
                    if self.drain_body(br).await.is_err() {
                        self.should_close = true;
                    }

                    self.pre_stop();
                }
            }
        } else {
            // should be impossible
            self.should_close = true;
        }
    }

    async fn reply_auth_error<CDW>(&mut self, clt_w: &mut HttpClientWriter<CDW>)
    where
        CDW: AsyncWrite + Unpin,
    {
        let result = HttpProxyClientResponse::reply_auth_err(
            self.req.version,
            clt_w,
            &self.ctx.server_config.auth_realm,
            self.should_close,
        )
        .await;
        if result.is_err() {
            self.should_close = true;
        }
    }

    async fn drain_body<CDR>(&mut self, clt_r: &mut HttpClientReader<CDR>) -> ServerTaskResult<()>
    where
        CDR: AsyncRead + Unpin,
    {
        let mut body_reader = HttpBodyReader::new(
            clt_r,
            self.req.body_type().unwrap(),
            self.ctx.server_config.body_line_max_len,
        );
        let mut sink_w = tokio::io::sink();
        let mut clt_to_sink = LimitedCopy::new(
            &mut body_reader,
            &mut sink_w,
            &self.ctx.server_config.tcp_copy,
        );

        let idle_duration = self.ctx.server_config.task_idle_check_duration;
        let mut idle_interval =
            tokio::time::interval_at(Instant::now() + idle_duration, idle_duration);
        let mut idle_count = 0;
        loop {
            tokio::select! {
                biased;

                r = &mut clt_to_sink => {
                    return match r {
                        Ok(_) => Ok(()),
                        Err(LimitedCopyError::ReadFailed(e)) => Err(ServerTaskError::ClientTcpReadFailed(e)),
                        Err(LimitedCopyError::WriteFailed(_)) => Err(ServerTaskError::InternalServerError("write to sinking failed")),
                    };
                }
                _ = idle_interval.tick() => {
                    if clt_to_sink.is_idle() {
                        idle_count += 1;

                        if idle_count >= self.ctx.server_config.task_idle_max_count {
                            return if clt_to_sink.no_cached_data() {
                                Err(ServerTaskError::ClientAppTimeout("idle while reading request body"))
                            } else {
                                Err(ServerTaskError::InternalServerError("idle while writing to sinking"))
                            };
                        }
                    } else {
                        idle_count = 0;

                        clt_to_sink.reset_active();
                    }

                    if self.ctx.server_quit_policy.force_quit() {
                        return Err(ServerTaskError::CanceledAsServerQuit)
                    }
                }
            }
        }
    }
}
