/*
 * SPDX-License-Identifier: Apache-2.0
 * Copyright 2024-2025 ByteDance and/or its affiliates.
 */

use std::time::Duration;

use anyhow::anyhow;
use slog::slog_info;
use tokio::io::AsyncWriteExt;

use g3_daemon::server::ServerQuitPolicy;
use g3_dpi::ProtocolInspectAction;
use g3_imap_proto::CommandPipeline;
use g3_imap_proto::response::ByeResponse;
use g3_io_ext::{IdleInterval, LimitedCopyConfig, LineRecvVec, OnceBufReader};
use g3_slog_types::{LtUpstreamAddr, LtUuid};
use g3_types::net::UpstreamAddr;

use super::StartTlsProtocol;
#[cfg(feature = "quic")]
use crate::audit::DetourAction;
use crate::auth::User;
use crate::config::server::ServerConfig;
use crate::inspect::{
    BoxAsyncRead, BoxAsyncWrite, StreamInspectContext, StreamInspection, StreamTransitTask,
};
use crate::log::task::TaskEvent;
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
        if let Some(logger) = $obj.ctx.intercept_logger() {
            slog_info!(logger, $($args)+;
                "intercept_type" => "SmtpConnection",
                "task_id" => LtUuid($obj.ctx.server_task_id()),
                "depth" => $obj.ctx.inspection_depth,
                "upstream" => LtUpstreamAddr(&$obj.upstream),
                "server_bye" => $obj.server_bye,
                "client_logout" => $obj.client_logout,
            );
        }
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

impl<SC: ServerConfig> ImapInterceptObject<SC> {
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

    fn log_partial_shutdown(&self, task_event: TaskEvent) {
        if let Some(logger) = self.ctx.intercept_logger() {
            slog_info!(logger, "";
                "intercept_type" => "SmtpConnection",
                "task_id" => LtUuid(self.ctx.server_task_id()),
                "task_event" => task_event.as_str(),
                "depth" => self.ctx.inspection_depth,
                "upstream" => LtUpstreamAddr(&self.upstream),
            );
        }
    }
}

impl<SC: ServerConfig> StreamTransitTask for ImapInterceptObject<SC> {
    fn copy_config(&self) -> LimitedCopyConfig {
        self.ctx.server_config.limited_copy_config()
    }

    fn idle_check_interval(&self) -> IdleInterval {
        self.ctx.idle_wheel.register()
    }

    fn max_idle_count(&self) -> usize {
        self.ctx.max_idle_count
    }

    fn log_client_shutdown(&self) {
        self.log_partial_shutdown(TaskEvent::ClientShutdown);
    }

    fn log_upstream_shutdown(&self) {
        self.log_partial_shutdown(TaskEvent::UpstreamShutdown);
    }

    fn log_periodic(&self) {
        // TODO
    }

    fn log_flush_interval(&self) -> Option<Duration> {
        self.ctx.server_config.task_log_flush_interval()
    }

    fn quit_policy(&self) -> &ServerQuitPolicy {
        self.ctx.server_quit_policy.as_ref()
    }

    fn user(&self) -> Option<&User> {
        self.ctx.user()
    }
}

impl<SC> ImapInterceptObject<SC>
where
    SC: ServerConfig + Send + Sync + 'static,
{
    pub(crate) async fn intercept(mut self) -> ServerTaskResult<Option<StreamInspection<SC>>> {
        let r = match self.ctx.imap_inspect_action(self.upstream.host()) {
            ProtocolInspectAction::Intercept => self.do_intercept().await,
            #[cfg(feature = "quic")]
            ProtocolInspectAction::Detour => self.do_detour().await.map(|_| None),
            ProtocolInspectAction::Bypass => self.do_bypass().await.map(|_| None),
            ProtocolInspectAction::Block => self.do_block().await.map(|_| None),
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

        let mut detour_stream = match client.open_detour_stream().await {
            Ok(s) => s,
            Err(e) => {
                self.close_on_detour_error().await;
                return Err(ServerTaskError::InternalAdapterError(e));
            }
        };

        let detour_ctx = client.build_context(
            &self.ctx.server_config,
            &self.ctx.server_quit_policy,
            &self.ctx.idle_wheel,
            &self.ctx.task_notes,
            &self.upstream,
            g3_dpi::Protocol::Imap,
        );

        match detour_ctx.check_detour_action(&mut detour_stream).await {
            Ok(DetourAction::Continue) => {
                let ImapIo {
                    clt_r,
                    clt_w,
                    ups_r,
                    ups_w,
                } = self.io.take().unwrap();

                detour_ctx
                    .relay(clt_r, clt_w, ups_r, ups_w, detour_stream)
                    .await
            }
            Ok(DetourAction::Bypass) => {
                detour_stream.finish();
                self.do_bypass().await
            }
            Ok(DetourAction::Block) => {
                detour_stream.finish();
                self.do_block().await
            }
            Err(e) => {
                detour_stream.finish();
                self.close_on_detour_error().await;
                Err(ServerTaskError::InternalAdapterError(e))
            }
        }
    }

    #[cfg(feature = "quic")]
    async fn close_on_detour_error(&mut self) {
        let ImapIo {
            clt_r: _,
            mut clt_w,
            ups_r: _,
            mut ups_w,
        } = self.io.take().unwrap();

        tokio::spawn(async move {
            let _ = ups_w.shutdown().await;
        });

        if ByeResponse::reply_internal_error(&mut clt_w).await.is_ok() {
            let _ = clt_w.shutdown().await;
        }
    }

    async fn do_bypass(&mut self) -> ServerTaskResult<()> {
        let ImapIo {
            clt_r,
            clt_w,
            ups_r,
            ups_w,
        } = self.io.take().unwrap();

        self.transit_transparent(clt_r, clt_w, ups_r, ups_w).await
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
        clt_w
            .shutdown()
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
                    self.transit_transparent(clt_r, clt_w, ups_r, ups_w)
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
                let _ = ups_w.shutdown().await;
                let _ = clt_w.shutdown().await;
                Ok(())
            }
            CloseReason::Server => {
                self.mark_close_by_server();
                let _ = ups_w.shutdown().await;
                let _ = clt_w.shutdown().await;
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
