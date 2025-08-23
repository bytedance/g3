/*
 * SPDX-License-Identifier: Apache-2.0
 * Copyright 2023-2025 ByteDance and/or its affiliates.
 */

use std::time::Duration;

use anyhow::anyhow;
use slog::slog_info;
use tokio::io::AsyncWriteExt;

use g3_daemon::server::ServerQuitPolicy;
use g3_dpi::ProtocolInspectAction;
use g3_io_ext::{IdleInterval, LimitedWriteExt, StreamCopyConfig};
use g3_slog_types::{LtHttpHeaderValue, LtUpstreamAddr, LtUuid};
use g3_types::net::{UpstreamAddr, WebSocketNotes};

use super::{ClientCloseFrame, ServerCloseFrame};
#[cfg(feature = "quic")]
use crate::audit::DetourAction;
use crate::auth::User;
use crate::config::server::ServerConfig;
use crate::inspect::{BoxAsyncRead, BoxAsyncWrite, StreamInspectContext, StreamTransitTask};
use crate::log::task::TaskEvent;
use crate::serve::{ServerTaskError, ServerTaskResult};

macro_rules! intercept_log {
    ($obj:tt, $($args:tt)+) => {
        if let Some(logger) = $obj.ctx.intercept_logger() {
            slog_info!(logger, $($args)+;
                "intercept_type" => "H1Websocket",
                "task_id" => LtUuid($obj.ctx.server_task_id()),
                "depth" => $obj.ctx.inspection_depth,
                "upstream" => LtUpstreamAddr(&$obj.upstream),
                "ws_resource_name" => $obj.ws_notes.resource_name(),
                "ws_origin" => $obj.ws_notes.origin().map(LtHttpHeaderValue),
                "ws_sub_protocol" => $obj.ws_notes.sub_protocol().map(LtHttpHeaderValue),
                "ws_version" => $obj.ws_notes.version().map(LtHttpHeaderValue),
            );
        }
    };
}

struct H1WebsocketIo {
    pub(crate) clt_r: BoxAsyncRead,
    pub(crate) clt_w: BoxAsyncWrite,
    pub(crate) ups_r: BoxAsyncRead,
    pub(crate) ups_w: BoxAsyncWrite,
}

pub(crate) struct H1WebsocketInterceptObject<SC: ServerConfig> {
    io: Option<H1WebsocketIo>,
    pub(crate) ctx: StreamInspectContext<SC>,
    upstream: UpstreamAddr,
    ws_notes: WebSocketNotes,
}

impl<SC: ServerConfig> H1WebsocketInterceptObject<SC> {
    pub(crate) fn new(
        ctx: StreamInspectContext<SC>,
        upstream: UpstreamAddr,
        ws_notes: WebSocketNotes,
    ) -> Self {
        H1WebsocketInterceptObject {
            io: None,
            ctx,
            upstream,
            ws_notes,
        }
    }

    pub(crate) fn set_io(
        &mut self,
        clt_r: BoxAsyncRead,
        clt_w: BoxAsyncWrite,
        ups_r: BoxAsyncRead,
        ups_w: BoxAsyncWrite,
    ) {
        let io = H1WebsocketIo {
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
                "intercept_type" => "H1Websocket",
                "task_id" => LtUuid(self.ctx.server_task_id()),
                "task_event" => task_event.as_str(),
                "depth" => self.ctx.inspection_depth,
                "upstream" => LtUpstreamAddr(&self.upstream),
                "ws_resource_name" => self.ws_notes.resource_name(),
                "ws_origin" => self.ws_notes.origin().map(LtHttpHeaderValue),
                "ws_sub_protocol" => self.ws_notes.sub_protocol().map(LtHttpHeaderValue),
                "ws_version" => self.ws_notes.version().map(LtHttpHeaderValue),
            );
        }
    }
}

impl<SC: ServerConfig> StreamTransitTask for H1WebsocketInterceptObject<SC> {
    fn copy_config(&self) -> StreamCopyConfig {
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

impl<SC: ServerConfig> H1WebsocketInterceptObject<SC> {
    pub(crate) async fn intercept(mut self) -> ServerTaskResult<()> {
        let r = match self.ctx.websocket_inspect_action(self.upstream.host()) {
            ProtocolInspectAction::Intercept => self.do_intercept().await,
            #[cfg(feature = "quic")]
            ProtocolInspectAction::Detour => self.do_detour().await,
            ProtocolInspectAction::Bypass => self.do_bypass().await,
            ProtocolInspectAction::Block => self.do_block().await,
        };
        match r {
            Ok(_) => {
                intercept_log!(self, "finished");
                Ok(())
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

        let mut detour_ctx = client.build_context(
            &self.ctx.server_config,
            &self.ctx.server_quit_policy,
            &self.ctx.idle_wheel,
            &self.ctx.task_notes,
            &self.upstream,
            g3_dpi::Protocol::Websocket,
        );
        detour_ctx.set_payload(self.ws_notes.serialize());

        match detour_ctx.check_detour_action(&mut detour_stream).await {
            Ok(DetourAction::Continue) => {
                let H1WebsocketIo {
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
        const SERVER_CLOSE_BYTES: [u8; 4] = ServerCloseFrame::encode_with_status_code(1011);
        const CLIENT_CLOSE_BYTES: [u8; 8] = ClientCloseFrame::encode_with_status_code(1001);

        let H1WebsocketIo {
            clt_r: _,
            mut clt_w,
            ups_r: _,
            mut ups_w,
        } = self.io.take().unwrap();

        tokio::spawn(async move {
            if ups_w.write_all_flush(&CLIENT_CLOSE_BYTES).await.is_ok() {
                let _ = ups_w.shutdown().await;
            }
        });

        if clt_w.write_all_flush(&SERVER_CLOSE_BYTES).await.is_ok() {
            let _ = clt_w.shutdown().await;
        }
    }

    async fn do_bypass(&mut self) -> ServerTaskResult<()> {
        let H1WebsocketIo {
            clt_r,
            clt_w,
            ups_r,
            ups_w,
        } = self.io.take().unwrap();

        self.transit_transparent(clt_r, clt_w, ups_r, ups_w).await
    }

    async fn do_block(&mut self) -> ServerTaskResult<()> {
        const SERVER_CLOSE_BYTES: [u8; 4] = ServerCloseFrame::encode_with_status_code(1001);
        const CLIENT_CLOSE_BYTES: [u8; 8] = ClientCloseFrame::encode_with_status_code(1001);

        let H1WebsocketIo {
            clt_r: _,
            mut clt_w,
            ups_r: _,
            mut ups_w,
        } = self.io.take().unwrap();

        tokio::spawn(async move {
            if ups_w.write_all_flush(&CLIENT_CLOSE_BYTES).await.is_ok() {
                let _ = ups_w.shutdown().await;
            }
        });

        if clt_w.write_all_flush(&SERVER_CLOSE_BYTES).await.is_ok() {
            let _ = clt_w.shutdown().await;
        }
        Err(ServerTaskError::InternalAdapterError(anyhow!(
            "websocket blocked by inspection policy"
        )))
    }

    async fn do_intercept(&mut self) -> ServerTaskResult<()> {
        let H1WebsocketIo {
            clt_r,
            clt_w,
            ups_r,
            ups_w,
        } = self.io.take().unwrap();

        self.transit_transparent(clt_r, clt_w, ups_r, ups_w).await
    }
}
