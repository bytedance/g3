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

use async_recursion::async_recursion;
use http::{Method, Version};
use slog::slog_info;
use tokio::io::{AsyncRead, AsyncWrite};

use g3_daemon::log::types::LtUuid;
use g3_dpi::Protocol;
use g3_io_ext::FlexBufReader;

use crate::config::server::ServerConfig;
use crate::inspect::{
    BoxAsyncRead, BoxAsyncWrite, InterceptionError, StreamInspectContext, StreamInspection,
};
use crate::module::http_forward::HttpProxyClientResponse;
use crate::serve::ServerTaskResult;

mod error;
pub(crate) use error::H1InterceptionError;

mod pipeline;
use pipeline::{HttpRecvRequest, HttpRequest, PipelineStats};

mod connect;
use connect::H1ConnectTask;

mod forward;
use forward::H1ForwardTask;

mod upgrade;
use upgrade::H1UpgradeTask;

pub(crate) struct HttpRequestIo<R: AsyncRead, W: AsyncWrite> {
    clt_r: FlexBufReader<R>,
    ups_w: W,
}

pub(crate) struct HttpResponseIo<R: AsyncRead, W: AsyncWrite> {
    ups_r: FlexBufReader<R>,
    clt_w: W,
}

struct H1InterceptIo {
    clt_r: FlexBufReader<BoxAsyncRead>,
    clt_w: BoxAsyncWrite,
    ups_r: BoxAsyncRead,
    ups_w: BoxAsyncWrite,
}

pub(crate) struct H1InterceptObject<SC: ServerConfig> {
    io: Option<H1InterceptIo>,
    ctx: StreamInspectContext<SC>,
    req_id: usize,
}

impl<SC: ServerConfig> H1InterceptObject<SC> {
    pub(crate) fn new(ctx: StreamInspectContext<SC>) -> Self {
        H1InterceptObject {
            io: None,
            ctx,
            req_id: 0,
        }
    }

    pub(crate) fn set_io(
        &mut self,
        clt_r: FlexBufReader<BoxAsyncRead>,
        clt_w: BoxAsyncWrite,
        ups_r: BoxAsyncRead,
        ups_w: BoxAsyncWrite,
    ) {
        let io = H1InterceptIo {
            clt_r,
            clt_w,
            ups_r,
            ups_w,
        };
        self.io = Some(io);
    }
}

macro_rules! intercept_log {
    ($obj:tt, $($args:tt)+) => {
        slog_info!($obj.ctx.intercept_logger(), $($args)+;
            "intercept_type" => "H1Connection",
            "task_id" => LtUuid($obj.ctx.server_task_id()),
            "depth" => $obj.ctx.inspection_depth,
            "current_req_id" => $obj.req_id,
        )
    };
}

impl<SC> H1InterceptObject<SC>
where
    SC: ServerConfig + Send + Sync + 'static,
{
    pub(crate) async fn intercept(mut self) -> ServerTaskResult<Option<StreamInspection<SC>>> {
        match self.do_intercept().await {
            Ok(v) => {
                intercept_log!(self, "finished");
                Ok(v)
            }
            Err(e) => {
                intercept_log!(self, "{e}");
                Err(InterceptionError::H1(e).into_server_task_error(Protocol::Http1))
            }
        }
    }

    #[async_recursion]
    async fn do_intercept(&mut self) -> Result<Option<StreamInspection<SC>>, H1InterceptionError> {
        let H1InterceptIo {
            clt_r,
            clt_w,
            ups_r,
            ups_w,
        } = self.io.take().unwrap();
        let pipeline_stats = Arc::new(PipelineStats::default());

        let mut rsp_io = HttpResponseIo {
            ups_r: FlexBufReader::new(ups_r),
            clt_w,
        };
        let req_io = HttpRequestIo { clt_r, ups_w };
        let (req_forwarder, mut req_acceptor) =
            pipeline::new_request_handler(self.ctx.clone(), req_io, pipeline_stats.clone());
        tokio::spawn(async move { req_forwarder.into_running().await });

        while let Some(r) = req_acceptor.accept().await {
            self.req_id += 1;
            match r {
                HttpRecvRequest::ClientConnectionError(e) => return Err(e),
                HttpRecvRequest::ClientRequestError(e) => {
                    if let Some(rsp) =
                        HttpProxyClientResponse::from_request_error(&e, Version::HTTP_11)
                    {
                        let _ = rsp.reply_err_to_request(&mut rsp_io.clt_w).await;
                    }
                    return Err(e.into());
                }
                HttpRecvRequest::UpstreamWriteError(e) => {
                    // just close the connection
                    return Err(e);
                }
                HttpRecvRequest::RequestWithoutIo(r) => {
                    let mut forward_task = H1ForwardTask::new(self.ctx.clone(), &r, self.req_id);
                    forward_task.forward_without_body(&mut rsp_io).await;
                    pipeline_stats.del_task();
                    if forward_task.should_close() {
                        req_acceptor.close();
                    }
                }
                HttpRecvRequest::RequestWithIO(r, mut req_io, io_sender) => {
                    if r.inner.method == Method::CONNECT {
                        let mut connect_task = H1ConnectTask::new(self.ctx.clone(), r, self.req_id);
                        // TODO check if allowed by adapter
                        if let Some(upstream) = connect_task.recv_connect(&mut rsp_io).await {
                            pipeline_stats.del_task();

                            let next_obj = connect_task.into_connect(req_io, rsp_io, upstream);
                            return Ok(Some(next_obj));
                        } else if connect_task.should_close() {
                            pipeline_stats.del_task();

                            req_acceptor.close();
                        } else {
                            pipeline_stats.del_task();
                        }
                    } else if r.inner.upgrade {
                        let mut upgrade_task = H1UpgradeTask::new(self.ctx.clone(), r, self.req_id);
                        // TODO check if allowed by adapter
                        if let Some((protocol, upstream)) =
                            upgrade_task.recv_upgrade(&mut rsp_io).await
                        {
                            pipeline_stats.del_task();

                            let next_obj =
                                upgrade_task.into_upgrade(req_io, rsp_io, protocol, upstream)?;
                            return Ok(Some(next_obj));
                        } else if upgrade_task.should_close() {
                            pipeline_stats.del_task();

                            req_acceptor.close();
                        } else {
                            pipeline_stats.del_task();
                        }
                    } else {
                        let mut forward_task =
                            H1ForwardTask::new(self.ctx.clone(), &r, self.req_id);
                        if let Some(reqmod_client) = self.ctx.audit_handle.icap_reqmod_client() {
                            forward_task
                                .adapt_with_io(&mut req_io, &mut rsp_io, reqmod_client)
                                .await;
                        } else {
                            forward_task.forward_with_io(&mut req_io, &mut rsp_io).await;
                        }
                        pipeline_stats.del_task();
                        if forward_task.should_close() {
                            req_acceptor.close();
                        } else {
                            let _ = io_sender.send(req_io).await;
                        }
                    }
                }
            }
        }

        Ok(None)
    }
}

fn convert_io(
    req_io: HttpRequestIo<BoxAsyncRead, BoxAsyncWrite>,
    rsp_io: HttpResponseIo<BoxAsyncRead, BoxAsyncWrite>,
) -> (BoxAsyncRead, BoxAsyncWrite, BoxAsyncRead, BoxAsyncWrite) {
    let HttpRequestIo { clt_r, ups_w } = req_io;
    let HttpResponseIo { ups_r, clt_w } = rsp_io;

    let clt_r = if clt_r.buffer().is_empty() {
        clt_r.into_inner()
    } else {
        Box::new(clt_r) as BoxAsyncRead
    };

    let ups_r = if ups_r.buffer().is_empty() {
        ups_r.into_inner()
    } else {
        Box::new(ups_r) as BoxAsyncRead
    };

    (clt_r, clt_w, ups_r, ups_w)
}
