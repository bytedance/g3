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

use anyhow::anyhow;
use slog::slog_info;
use tokio::io::AsyncWriteExt;

use g3_dpi::ProtocolInspectPolicy;
use g3_imap_proto::response::ByeResponse;
use g3_imap_proto::CommandPipeline;
use g3_io_ext::{LineRecvVec, OnceBufReader};
use g3_slog_types::{LtUpstreamAddr, LtUuid};
use g3_types::net::UpstreamAddr;

use super::StartTlsProtocol;
#[cfg(feature = "quic")]
use crate::audit::StreamDetourContext;
use crate::config::server::ServerConfig;
use crate::inspect::{BoxAsyncRead, BoxAsyncWrite, StreamInspectContext, StreamInspection};
use crate::serve::{ServerTaskError, ServerTaskResult};

mod ext;
use ext::{CommandLineReceiveExt, ResponseLineReceiveExt};

mod capability;
use capability::Capability;

mod greeting;
use greeting::Greeting;

mod not_authenticated;
use not_authenticated::InitiationStatus;

mod authenticated;
use authenticated::CloseReason;

mod forward;
use forward::ResponseAction;

mod logout;

struct ImapRelayBuf {
    rsp_recv_buf: LineRecvVec,
    cmd_recv_buf: LineRecvVec,
}

macro_rules! intercept_log {
    ($obj:tt, $($args:tt)+) => {
        slog_info!($obj.ctx.intercept_logger(), $($args)+;
            "intercept_type" => "SmtpConnection",
            "task_id" => LtUuid($obj.ctx.server_task_id()),
            "depth" => $obj.ctx.inspection_depth,
            "upstream" => LtUpstreamAddr(&$obj.upstream),
            "server_bye" => $obj.server_bye,
            "client_logout" => $obj.client_logout,
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
    cmd_pipeline: CommandPipeline,
    server_bye: bool,
    client_logout: bool,
    authenticated: bool,
    mailbox_selected: bool,
    capability: Capability,
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
            cmd_pipeline: CommandPipeline::default(),
            server_bye: false,
            client_logout: false,
            authenticated: false,
            mailbox_selected: false,
            capability: Capability::default(),
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
        let r = match self.ctx.imap_inspect_policy() {
            ProtocolInspectPolicy::Intercept => self.do_intercept().await,
            #[cfg(feature = "quic")]
            ProtocolInspectPolicy::Detour => self.do_detour().await.map(|_| None),
            ProtocolInspectPolicy::Bypass => self.do_bypass().await.map(|_| None),
            ProtocolInspectPolicy::Block => self.do_block().await.map(|_| None),
        };
        match r {
            Ok(obj) => {
                intercept_log!(self, "finished");
                Ok(obj)
            }
            Err(e) => {
                intercept_log!(self, "{e}");
                Err(e)
            }
        }
    }

    #[cfg(feature = "quic")]
    async fn do_detour(&mut self) -> ServerTaskResult<()> {
        let Some(client) = self.ctx.audit_handle.stream_detour_client() else {
            return self.do_bypass().await;
        };

        let ImapIo {
            clt_r,
            clt_w,
            ups_r,
            ups_w,
        } = self.io.take().unwrap();

        let ctx = StreamDetourContext::new(
            &self.ctx.server_config,
            &self.ctx.server_quit_policy,
            &self.ctx.task_notes,
            &self.upstream,
            g3_dpi::Protocol::Imap,
        );

        client.detour_relay(clt_r, clt_w, ups_r, ups_w, ctx).await
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
        Err(ServerTaskError::InternalAdapterError(anyhow!(
            "imap blocked by inspection policy"
        )))
    }

    fn mark_close_by_server(&mut self) {
        self.server_bye = true;
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
        let mut relay_buf = ImapRelayBuf {
            rsp_recv_buf,
            cmd_recv_buf: LineRecvVec::with_capacity(interception_config.command_line_max_size),
        };

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
            self.mark_close_by_server();
            return Ok(None);
        }
        if greeting.pre_authenticated() {
            self.capability = greeting.into_capability();
            self.enter_authenticated(clt_r, clt_w, ups_r, ups_w, relay_buf)
                .await?;
            Ok(None)
        } else {
            self.capability = greeting.into_capability();
            self.start_initiation(clt_r, clt_w, ups_r, ups_w, relay_buf)
                .await
        }
    }

    async fn start_initiation(
        &mut self,
        mut clt_r: BoxAsyncRead,
        mut clt_w: BoxAsyncWrite,
        mut ups_r: BoxAsyncRead,
        mut ups_w: BoxAsyncWrite,
        mut relay_buf: ImapRelayBuf,
    ) -> ServerTaskResult<Option<StreamInspection<SC>>> {
        match self
            .relay_not_authenticated(
                &mut clt_r,
                &mut clt_w,
                &mut ups_r,
                &mut ups_w,
                &mut relay_buf,
            )
            .await?
        {
            InitiationStatus::ClientClose => {
                self.handle_client_logout(&mut clt_w, &mut ups_r, &mut relay_buf.rsp_recv_buf)
                    .await?;
                Ok(None)
            }
            InitiationStatus::ServerClose => {
                self.mark_close_by_server();
                Ok(None)
            }
            InitiationStatus::LocalClose(e) => {
                self.start_server_logout(&mut ups_r, &mut ups_w, &mut relay_buf.rsp_recv_buf)
                    .await;
                Err(e)
            }
            InitiationStatus::StartTls => {
                if let Some(tls_interception) = self.ctx.tls_interception() {
                    let mut start_tls_obj = crate::inspect::start_tls::StartTlsInterceptObject::new(
                        self.ctx.clone(),
                        self.upstream.clone(),
                        tls_interception,
                        StartTlsProtocol::Imap,
                    );
                    start_tls_obj.set_io(clt_r, clt_w, ups_r, ups_w);
                    Ok(Some(StreamInspection::StartTls(start_tls_obj)))
                } else {
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
                    .map(|_| None)
                }
            }
            InitiationStatus::Authenticated => {
                self.enter_authenticated(clt_r, clt_w, ups_r, ups_w, relay_buf)
                    .await?;
                Ok(None)
            }
        }
    }

    async fn enter_authenticated(
        &mut self,
        mut clt_r: BoxAsyncRead,
        mut clt_w: BoxAsyncWrite,
        mut ups_r: BoxAsyncRead,
        mut ups_w: BoxAsyncWrite,
        mut relay_buf: ImapRelayBuf,
    ) -> ServerTaskResult<()> {
        match self
            .relay_authenticated(
                &mut clt_r,
                &mut clt_w,
                &mut ups_r,
                &mut ups_w,
                &mut relay_buf,
            )
            .await?
        {
            CloseReason::Client => {
                self.handle_client_logout(&mut clt_w, &mut ups_r, &mut relay_buf.rsp_recv_buf)
                    .await?;
                Ok(())
            }
            CloseReason::Server => {
                self.mark_close_by_server();
                Ok(())
            }
            CloseReason::Local(e) => {
                self.start_server_logout(&mut ups_r, &mut ups_w, &mut relay_buf.rsp_recv_buf)
                    .await;
                Err(e)
            }
        }
    }
}
