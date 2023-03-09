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

use log::trace;
use tokio::io::AsyncRead;
use tokio::sync::mpsc;

use g3_io_ext::{LimitedBufReadExt, LimitedBufReader, NilLimitedReaderStats};
use g3_types::net::{HttpForwardedHeaderType, HttpForwardedHeaderValue};

use super::protocol::{HttpClientReader, HttpRProxyRequest};
use super::{CommonTaskContext, HttpRProxyCltWrapperStats, HttpRProxyPipelineStats};
use crate::module::http_forward::HttpProxyClientResponse;
use crate::serve::ServerStats;

pub(crate) struct HttpRProxyPipelineReaderTask<CDR> {
    ctx: Arc<CommonTaskContext>,
    task_queue: mpsc::Sender<Result<HttpRProxyRequest<CDR>, HttpProxyClientResponse>>,
    stream_reader: Option<HttpClientReader<CDR>>,
    pipeline_stats: Arc<HttpRProxyPipelineStats>,
}

impl<CDR> HttpRProxyPipelineReaderTask<CDR>
where
    CDR: AsyncRead + Send + Unpin + 'static,
{
    pub(crate) fn new(
        ctx: &Arc<CommonTaskContext>,
        task_sender: mpsc::Sender<Result<HttpRProxyRequest<CDR>, HttpProxyClientResponse>>,
        read_half: CDR,
        pipeline_stats: &Arc<HttpRProxyPipelineStats>,
    ) -> Self {
        let clt_r_stats = HttpRProxyCltWrapperStats::new_for_reader(&ctx.server_stats);
        let limit_config = &ctx.server_config.tcp_sock_speed_limit;
        let clt_r = LimitedBufReader::new(
            read_half,
            limit_config.shift_millis,
            limit_config.max_north,
            clt_r_stats,
            Arc::new(NilLimitedReaderStats::default()),
        );
        HttpRProxyPipelineReaderTask {
            ctx: Arc::clone(ctx),
            task_queue: task_sender,
            stream_reader: Some(clt_r),
            pipeline_stats: Arc::clone(pipeline_stats),
        }
    }

    pub(crate) async fn into_running(mut self) {
        // NOTE the receiver end should not be cloned, as the closed events is bounding to each
        let task_queue = self.task_queue.clone(); // to avoid ref self
        tokio::select! {
            biased;

            _ = task_queue.closed() => {
                trace!("write end has closed for previous request");
            }
            _ = self.run() => {}
        }
    }

    fn append_forwarded(&self, req: &mut HttpRProxyRequest<CDR>) {
        match self.ctx.server_config.append_forwarded_for {
            HttpForwardedHeaderType::Disable => {}
            HttpForwardedHeaderType::Classic => {
                let v = HttpForwardedHeaderValue::new_classic(self.ctx.tcp_client_addr.ip());
                v.append_to(&mut req.inner.end_to_end_headers);
            }
            HttpForwardedHeaderType::Standard => {
                let v = HttpForwardedHeaderValue::new_standard(
                    self.ctx.tcp_client_addr,
                    self.ctx.tcp_server_addr,
                );
                v.append_to(&mut req.inner.end_to_end_headers);
            }
        }
    }

    async fn run(&mut self) {
        let (stream_sender, mut stream_receiver) = mpsc::channel(1);
        loop {
            if let Some(mut reader) = self.stream_reader.take() {
                let quit_after_timeout = self.pipeline_stats.get_alive_task() <= 0;

                match tokio::time::timeout(
                    self.ctx.server_config.pipeline_read_idle_timeout,
                    reader.fill_wait_data(),
                )
                .await
                {
                    Ok(Ok(true)) => {}
                    Ok(Ok(false)) => {
                        trace!("client {} closed", self.ctx.tcp_client_addr);
                        break;
                    }
                    Ok(Err(e)) => {
                        trace!(
                            "client {} closed with error {:?}",
                            self.ctx.tcp_client_addr,
                            e
                        );
                        break;
                    }
                    Err(_) => {
                        // timeout
                        self.stream_reader = Some(reader);
                        if quit_after_timeout {
                            // TODO may be attack
                            break;
                        }
                        continue;
                    }
                }

                let mut version: http::Version = http::Version::HTTP_11; // default to 1.1
                match tokio::time::timeout(
                    self.ctx.server_config.timeout.recv_req_header,
                    HttpRProxyRequest::parse(
                        &mut reader,
                        stream_sender.clone(),
                        self.ctx.server_config.req_hdr_max_size,
                        self.ctx.server_config.server_id.as_ref(),
                        &mut version,
                    ),
                )
                .await
                {
                    Ok(Ok((mut req, send_reader))) => {
                        self.append_forwarded(&mut req);

                        if send_reader {
                            req.body_reader = Some(reader);
                        } else {
                            self.stream_reader = Some(reader);
                        }

                        let server_is_online = self.ctx.server_stats.is_online();
                        if !server_is_online {
                            // According to https://datatracker.ietf.org/doc/html/rfc7230#section-6.3.2
                            // A client that pipelines requests SHOULD retry unanswered requests if
                            // the connection closes before it receives all of the corresponding
                            // responses.
                            req.inner.disable_keep_alive();
                        }

                        if self.task_queue.send(Ok(req)).await.is_err() {
                            trace!("write end has closed for previous request while sending new request");
                            break;
                        }
                        self.pipeline_stats.add_task();

                        if !server_is_online {
                            break;
                        }
                    }
                    Ok(Err(e)) => {
                        self.stream_reader = Some(reader);
                        if let Some(response) =
                            HttpProxyClientResponse::from_request_error(&e, version)
                        {
                            if self.task_queue.send(Err(response)).await.is_err() {
                                trace!("write end has closed for previous request while sending error response");
                            }
                        }
                        trace!(
                            "Error handling client {}: {:?}",
                            self.ctx.tcp_client_addr,
                            e
                        );
                        // TODO handle error, negotiation failed, may be attack
                        break;
                    }
                    Err(_) => {
                        trace!("timeout to read in a complete request header");
                        // TODO handle timeout, may be attack
                        break;
                    }
                }
            } else {
                match stream_receiver.recv().await.flatten() {
                    Some(mut reader) => {
                        // we can now read the next request
                        reader.reset_buffer_stats(Arc::new(NilLimitedReaderStats::default()));
                        let limit_config = &self.ctx.server_config.tcp_sock_speed_limit;
                        reader.reset_limit(limit_config.shift_millis, limit_config.max_north);
                        self.stream_reader = Some(reader);
                    }
                    None => {
                        // write end closed normally, task done
                        break;
                    }
                }
            }
        }
    }
}
