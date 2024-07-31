/*
 * Copyright 2024 ByteDance and/or its affiliates.
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

use slog::slog_info;
use tokio::io::AsyncWriteExt;

use g3_dpi::ProtocolInspectPolicy;
use g3_imap_proto::response::ByeResponse;
use g3_io_ext::{LineRecvVec, OnceBufReader};
use g3_slog_types::{LtUpstreamAddr, LtUuid};
use g3_types::net::UpstreamAddr;

use crate::config::server::ServerConfig;
use crate::inspect::{BoxAsyncRead, BoxAsyncWrite, StreamInspectContext, StreamInspection};
use crate::serve::{ServerTaskError, ServerTaskForbiddenError, ServerTaskResult};

mod greeting;
use greeting::Greeting;

struct ImapRelayBuf {
    rsp_recv_buf: LineRecvVec,
}

macro_rules! intercept_log {
    ($obj:tt, $($args:tt)+) => {
        slog_info!($obj.ctx.intercept_logger(), $($args)+;
            "intercept_type" => "SmtpConnection",
            "task_id" => LtUuid($obj.ctx.server_task_id()),
            "depth" => $obj.ctx.inspection_depth,
            "upstream" => LtUpstreamAddr(&$obj.upstream),
        )
    };
}

struct ImapIo {
    pub(crate) clt_r: BoxAsyncRead,
    pub(crate) clt_w: BoxAsyncWrite,
    pub(crate) ups_r: OnceBufReader<BoxAsyncRead>,
    pub(crate) ups_w: BoxAsyncWrite,
}

pub(crate) struct ImapInterceptObject<SC: ServerConfig> {
    io: Option<ImapIo>,
    ctx: StreamInspectContext<SC>,
    upstream: UpstreamAddr,
    from_starttls: bool,
}

impl<SC> ImapInterceptObject<SC>
where
    SC: ServerConfig + Send + Sync + 'static,
{
    pub(crate) fn new(ctx: StreamInspectContext<SC>, upstream: UpstreamAddr) -> Self {
        ImapInterceptObject {
            io: None,
            ctx,
            upstream,
            from_starttls: false,
        }
    }

    pub(crate) fn set_from_starttls(&mut self) {
        self.from_starttls = true;
    }

    pub(crate) fn set_io(
        &mut self,
        clt_r: BoxAsyncRead,
        clt_w: BoxAsyncWrite,
        ups_r: OnceBufReader<BoxAsyncRead>,
        ups_w: BoxAsyncWrite,
    ) {
        let io = ImapIo {
            clt_r,
            clt_w,
            ups_r,
            ups_w,
        };
        self.io = Some(io);
    }

    pub(crate) async fn intercept(mut self) -> ServerTaskResult<Option<StreamInspection<SC>>> {
        match self.ctx.imap_inspect_policy() {
            ProtocolInspectPolicy::Bypass => {
                self.do_bypass().await?;
                Ok(None)
            }
            ProtocolInspectPolicy::Intercept => match self.do_intercept().await {
                Ok(obj) => {
                    intercept_log!(self, "finished");
                    Ok(obj)
                }
                Err(e) => {
                    intercept_log!(self, "{e}");
                    Err(e)
                }
            },
            ProtocolInspectPolicy::Block => {
                self.do_block().await?;
                Ok(None)
            }
        }
    }

    async fn do_bypass(&mut self) -> ServerTaskResult<()> {
        let ImapIo {
            clt_r,
            clt_w,
            ups_r,
            ups_w,
        } = self.io.take().unwrap();

        crate::inspect::stream::transit_transparent(
            clt_r,
            clt_w,
            ups_r,
            ups_w,
            &self.ctx.server_config,
            &self.ctx.server_quit_policy,
            self.ctx.user(),
        )
        .await
    }

    async fn do_block(&mut self) -> ServerTaskResult<()> {
        let ImapIo {
            clt_r: _,
            mut clt_w,
            ups_r: _,
            mut ups_w,
        } = self.io.take().unwrap();

        tokio::spawn(async move {
            let _ = ups_w.shutdown().await;
        });

        ByeResponse::reply_blocked(&mut clt_w)
            .await
            .map_err(ServerTaskError::ClientTcpWriteFailed)?;
        Err(ServerTaskError::ForbiddenByRule(
            ServerTaskForbiddenError::ProtoBanned,
        ))
    }

    async fn do_intercept(&mut self) -> ServerTaskResult<Option<StreamInspection<SC>>> {
        let ImapIo {
            clt_r,
            mut clt_w,
            ups_r,
            ups_w,
        } = self.io.take().unwrap();

        let interception_config = self.ctx.imap_interception();

        let (initial_data, mut ups_r) = ups_r.into_parts();
        let rsp_recv_buf = if let Some(data) = initial_data {
            LineRecvVec::with_data(&data, interception_config.response_line_max_size)
        } else {
            LineRecvVec::with_capacity(interception_config.response_line_max_size)
        };
        let mut relay_buf = ImapRelayBuf { rsp_recv_buf };

        if self.from_starttls {
            return self
                .start_initiation(clt_r, clt_w, ups_r, ups_w, relay_buf)
                .await;
        }

        let mut greeting = Greeting::default();
        if let Err(e) = greeting
            .relay(
                &mut ups_r,
                &mut clt_w,
                &mut relay_buf.rsp_recv_buf,
                interception_config.greeting_timeout,
            )
            .await
        {
            greeting.reply_no_service(&e, &mut clt_w).await;
            return Err(e.into());
        }
        if greeting.close_service() {
            return Ok(None);
        }
        if greeting.pre_authenticated() {
            self.run_authenticated(clt_r, clt_w, ups_r, ups_w, relay_buf)
                .await?;
            Ok(None)
        } else {
            self.start_initiation(clt_r, clt_w, ups_r, ups_w, relay_buf)
                .await
        }
    }

    async fn start_initiation(
        &mut self,
        clt_r: BoxAsyncRead,
        clt_w: BoxAsyncWrite,
        ups_r: BoxAsyncRead,
        ups_w: BoxAsyncWrite,
        _relay_buf: ImapRelayBuf,
    ) -> ServerTaskResult<Option<StreamInspection<SC>>> {
        crate::inspect::stream::transit_transparent(
            clt_r,
            clt_w,
            ups_r,
            ups_w,
            &self.ctx.server_config,
            &self.ctx.server_quit_policy,
            self.ctx.user(),
        )
        .await?;

        Ok(None)
    }

    async fn run_authenticated(
        &mut self,
        clt_r: BoxAsyncRead,
        clt_w: BoxAsyncWrite,
        ups_r: BoxAsyncRead,
        ups_w: BoxAsyncWrite,
        _relay_buf: ImapRelayBuf,
    ) -> ServerTaskResult<()> {
        crate::inspect::stream::transit_transparent(
            clt_r,
            clt_w,
            ups_r,
            ups_w,
            &self.ctx.server_config,
            &self.ctx.server_quit_policy,
            self.ctx.user(),
        )
        .await
    }
}
