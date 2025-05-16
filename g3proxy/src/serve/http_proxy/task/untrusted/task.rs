/*
 * SPDX-License-Identifier: Apache-2.0
 * Copyright 2023-2025 ByteDance and/or its affiliates.
 */

use std::sync::Arc;

use tokio::io::{AsyncRead, AsyncWrite};

use g3_http::HttpBodyReader;
use g3_http::server::HttpProxyClientRequest;
use g3_io_ext::{LimitedCopy, LimitedCopyError};

use super::protocol::{HttpClientReader, HttpClientWriter, HttpProxyRequest};
use super::{CommonTaskContext, UntrustedCltReadWrapperStats};
use crate::module::http_forward::HttpProxyClientResponse;
use crate::serve::{ServerTaskError, ServerTaskResult};

pub(crate) struct HttpProxyUntrustedTask<'a> {
    ctx: Arc<CommonTaskContext>,
    req: &'a HttpProxyClientRequest,
    should_close: bool,
    started: bool,
}

impl Drop for HttpProxyUntrustedTask<'_> {
    fn drop(&mut self) {
        if self.started {
            self.post_stop();
            self.started = false;
        }
    }
}

impl<'a> HttpProxyUntrustedTask<'a> {
    pub(crate) fn new(
        ctx: &Arc<CommonTaskContext>,
        req: &'a HttpProxyRequest<impl AsyncRead>,
    ) -> Self {
        HttpProxyUntrustedTask {
            ctx: Arc::clone(ctx),
            req: &req.inner,
            should_close: !req.inner.keep_alive(),
            started: false,
        }
    }

    fn pre_start(&mut self) {
        self.ctx.server_stats.task_http_untrusted.add_task();
        self.ctx.server_stats.task_http_untrusted.inc_alive_task();

        self.started = true;
    }

    fn post_stop(&self) {
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
                    br.reset_local_limit(limit_config.shift_millis, limit_config.max_north);
                    let buffer_stats =
                        UntrustedCltReadWrapperStats::new_obj(&self.ctx.server_stats);
                    br.reset_buffer_stats(buffer_stats);

                    self.pre_start();
                    if self.drain_body(br).await.is_err() {
                        self.should_close = true;
                    }
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
        let result = HttpProxyClientResponse::reply_proxy_auth_err(
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

        let mut idle_interval = self.ctx.idle_wheel.register();
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
                n = idle_interval.tick() => {
                    if clt_to_sink.is_idle() {
                        idle_count += n;

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
