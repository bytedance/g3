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

use chrono::{DateTime, Utc};
use tokio::io::AsyncRead;
use tokio::sync::mpsc;
use tokio::time::Instant;

use g3_http::server::{HttpRequestParseError, HttpTransparentRequest};
use g3_io_ext::LimitedBufReadExt;

use super::{H1InterceptionError, HttpRequestIo, PipelineStats};
use crate::config::server::ServerConfig;
use crate::inspect::StreamInspectContext;

pub(crate) struct HttpRequest {
    pub(crate) inner: HttpTransparentRequest,
    pub(crate) time_received: Instant,
    pub(crate) datetime_received: DateTime<Utc>,
}

pub(crate) enum HttpRecvRequest<R: AsyncRead> {
    ClientConnectionClosed,
    ClientConnectionError(H1InterceptionError),
    ClientRequestError(HttpRequestParseError),
    RequestWithIO(
        HttpRequest,
        HttpRequestIo<R>,
        mpsc::Sender<HttpRequestIo<R>>,
    ),
    RequestWithoutIo(HttpRequest),
}

pub(crate) struct HttpRequestAcceptor<R: AsyncRead> {
    recv_request: mpsc::Receiver<HttpRecvRequest<R>>,
}

impl<R> HttpRequestAcceptor<R>
where
    R: AsyncRead,
{
    pub(crate) fn new(recv_request: mpsc::Receiver<HttpRecvRequest<R>>) -> Self {
        HttpRequestAcceptor { recv_request }
    }

    pub(crate) async fn accept(&mut self) -> Option<HttpRecvRequest<R>> {
        self.recv_request.recv().await
    }

    pub(crate) fn close(&mut self) {
        self.recv_request.close();
    }
}

pub(crate) struct HttpRequestForwarder<SC: ServerConfig, R: AsyncRead> {
    ctx: StreamInspectContext<SC>,
    io: Option<HttpRequestIo<R>>,
    send_request: mpsc::Sender<HttpRecvRequest<R>>,
    stats: Arc<PipelineStats>,
}

impl<SC, R> HttpRequestForwarder<SC, R>
where
    SC: ServerConfig,
    R: AsyncRead + Unpin,
{
    pub(crate) fn new(
        ctx: StreamInspectContext<SC>,
        req_io: HttpRequestIo<R>,
        send_request: mpsc::Sender<HttpRecvRequest<R>>,
        stats: Arc<PipelineStats>,
    ) -> Self {
        HttpRequestForwarder {
            ctx,
            io: Some(req_io),
            send_request,
            stats,
        }
    }

    pub(crate) async fn into_running(mut self) {
        let send_request = self.send_request.clone();
        tokio::select! {
            biased;

            _ = send_request.closed() => {}
            _ = self.run() => {}
        }
    }

    async fn run(&mut self) {
        let (io_sender, mut io_receiver) = mpsc::channel(1);
        let http_config = self.ctx.h1_interception();
        loop {
            if let Some(mut io) = self.io.take() {
                let quit_after_timeout = self.stats.get_alive_task() <= 0;

                match tokio::time::timeout(
                    http_config.pipeline_read_idle_timeout,
                    io.clt_r.fill_wait_data(),
                )
                .await
                {
                    Ok(Ok(true)) => {}
                    Ok(Ok(false)) => {
                        let _ = self
                            .send_request
                            .send(HttpRecvRequest::ClientConnectionClosed)
                            .await;
                        break;
                    }
                    Ok(Err(e)) => {
                        let connection_error = H1InterceptionError::ClientReadFailed(e);
                        let _ = self
                            .send_request
                            .send(HttpRecvRequest::ClientConnectionError(connection_error))
                            .await;
                        break;
                    }
                    Err(_) => {
                        if quit_after_timeout {
                            let connection_error = H1InterceptionError::ClientAppTimeout(
                                "pipeline wait request timeout",
                            );
                            let _ = self
                                .send_request
                                .send(HttpRecvRequest::ClientConnectionError(connection_error))
                                .await;
                            break;
                        } else {
                            self.io = Some(io);
                            continue;
                        }
                    }
                }

                match tokio::time::timeout(
                    http_config.req_head_recv_timeout,
                    HttpTransparentRequest::parse(
                        &mut io.clt_r,
                        http_config.req_head_max_size,
                        http_config.steal_forwarded_for,
                    ),
                )
                .await
                {
                    Ok(Ok((mut req, _head_bytes))) => {
                        let datetime_received = Utc::now();
                        let time_received = Instant::now();

                        if self.ctx.server_offline() {
                            // According to https://datatracker.ietf.org/doc/html/rfc7230#section-6.3.2
                            // A client that pipelines requests SHOULD retry unanswered requests if
                            // the connection closes before it receives all the corresponding
                            // responses.
                            req.disable_keep_alive();
                        }

                        let recv_req = if self.ctx.audit_handle.icap_reqmod_client().is_some() {
                            HttpRecvRequest::RequestWithIO(
                                HttpRequest {
                                    inner: req,
                                    time_received,
                                    datetime_received,
                                },
                                io,
                                io_sender.clone(),
                            )
                        } else if req.pipeline_safe() {
                            self.io = Some(io);
                            HttpRecvRequest::RequestWithoutIo(HttpRequest {
                                inner: req,
                                time_received,
                                datetime_received,
                            })
                        } else {
                            HttpRecvRequest::RequestWithIO(
                                HttpRequest {
                                    inner: req,
                                    time_received,
                                    datetime_received,
                                },
                                io,
                                io_sender.clone(),
                            )
                        };

                        let _ = self.send_request.send(recv_req).await;
                        self.stats.add_task();
                    }
                    Ok(Err(e)) => {
                        let _ = self
                            .send_request
                            .send(HttpRecvRequest::ClientRequestError(e))
                            .await;
                        break;
                    }
                    Err(_) => {
                        let connection_error =
                            H1InterceptionError::ClientAppTimeout("pipeline read request timeout");
                        let _ = self
                            .send_request
                            .send(HttpRecvRequest::ClientConnectionError(connection_error))
                            .await;
                        break;
                    }
                }
            } else {
                match io_receiver.recv().await {
                    Some(io) => {
                        // we can now read the next request
                        self.io = Some(io);
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
