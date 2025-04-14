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

use std::time::Duration;

use anyhow::anyhow;
use bytes::Bytes;
use h2::{RecvStream, SendStream};
use slog::slog_info;

use g3_daemon::server::ServerQuitPolicy;
use g3_dpi::ProtocolInspectAction;
use g3_h2::{H2StreamReader, H2StreamWriter};
use g3_io_ext::{IdleInterval, LimitedCopyConfig};
use g3_slog_types::{LtHttpHeaderValue, LtUpstreamAddr, LtUuid};
use g3_types::net::{UpstreamAddr, WebSocketNotes};

use super::{ClientCloseFrame, ServerCloseFrame};
#[cfg(feature = "quic")]
use crate::audit::DetourAction;
use crate::auth::User;
use crate::config::server::ServerConfig;
use crate::inspect::{StreamInspectContext, StreamTransitTask};
use crate::log::task::TaskEvent;
use crate::serve::{ServerTaskError, ServerTaskResult};

macro_rules! intercept_log {
    ($obj:tt, $($args:tt)+) => {
        if let Some(logger) = $obj.ctx.intercept_logger() {
            slog_info!(logger, $($args)+;
                "intercept_type" => "H2Websocket",
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

pub(crate) struct H2WebsocketInterceptObject<SC: ServerConfig> {
    ctx: StreamInspectContext<SC>,
    upstream: UpstreamAddr,
    ws_notes: WebSocketNotes,
}

impl<SC: ServerConfig> H2WebsocketInterceptObject<SC> {
    pub(crate) fn new(
        ctx: StreamInspectContext<SC>,
        upstream: UpstreamAddr,
        ws_notes: WebSocketNotes,
    ) -> Self {
        H2WebsocketInterceptObject {
            ctx,
            upstream,
            ws_notes,
        }
    }

    fn log_partial_shutdown(&self, task_event: TaskEvent) {
        if let Some(logger) = self.ctx.intercept_logger() {
            slog_info!(logger, "";
                "intercept_type" => "H2Websocket",
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

impl<SC: ServerConfig> StreamTransitTask for H2WebsocketInterceptObject<SC> {
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

impl<SC: ServerConfig> H2WebsocketInterceptObject<SC> {
    pub(crate) async fn intercept(
        mut self,
        clt_r: RecvStream,
        clt_w: SendStream<Bytes>,
        ups_r: RecvStream,
        ups_w: SendStream<Bytes>,
    ) {
        let r = match self.ctx.websocket_inspect_action(self.upstream.host()) {
            ProtocolInspectAction::Intercept => self.do_intercept(clt_r, clt_w, ups_r, ups_w).await,
            #[cfg(feature = "quic")]
            ProtocolInspectAction::Detour => self.do_detour(clt_r, clt_w, ups_r, ups_w).await,
            ProtocolInspectAction::Bypass => self.do_bypass(clt_r, clt_w, ups_r, ups_w).await,
            ProtocolInspectAction::Block => self.do_block(clt_w, ups_w).await,
        };
        match r {
            Ok(_) => {
                intercept_log!(self, "finished");
            }
            Err(e) => {
                intercept_log!(self, "{e}");
            }
        }
    }

    #[cfg(feature = "quic")]
    async fn do_detour(
        &mut self,
        clt_r: RecvStream,
        clt_w: SendStream<Bytes>,
        ups_r: RecvStream,
        ups_w: SendStream<Bytes>,
    ) -> ServerTaskResult<()> {
        let Some(client) = self.ctx.audit_handle.stream_detour_client() else {
            return self.do_bypass(clt_r, clt_w, ups_r, ups_w).await;
        };

        let mut detour_stream = match client.open_detour_stream().await {
            Ok(s) => s,
            Err(e) => {
                self.close_on_detour_error(clt_w, ups_w);
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
                let clt_r = H2StreamReader::new(clt_r);
                let clt_w = H2StreamWriter::new(clt_w);
                let ups_r = H2StreamReader::new(ups_r);
                let ups_w = H2StreamWriter::new(ups_w);

                detour_ctx
                    .relay(clt_r, clt_w, ups_r, ups_w, detour_stream)
                    .await
            }
            Ok(DetourAction::Bypass) => {
                detour_stream.finish();
                self.do_bypass(clt_r, clt_w, ups_r, ups_w).await
            }
            Ok(DetourAction::Block) => {
                detour_stream.finish();
                self.do_block(clt_w, ups_w).await
            }
            Err(e) => {
                detour_stream.finish();
                self.close_on_detour_error(clt_w, ups_w);
                Err(ServerTaskError::InternalAdapterError(e))
            }
        }
    }

    #[cfg(feature = "quic")]
    fn close_on_detour_error(
        &mut self,
        mut clt_w: SendStream<Bytes>,
        mut ups_w: SendStream<Bytes>,
    ) {
        const SERVER_CLOSE_BYTES: [u8; 4] = ServerCloseFrame::encode_with_status_code(1011);
        const CLIENT_CLOSE_BYTES: [u8; 8] = ClientCloseFrame::encode_with_status_code(1001);

        let _ = ups_w.send_data(Bytes::from_static(&CLIENT_CLOSE_BYTES), true);
        let _ = clt_w.send_data(Bytes::from_static(&SERVER_CLOSE_BYTES), true);
    }

    async fn do_bypass(
        &mut self,
        clt_r: RecvStream,
        clt_w: SendStream<Bytes>,
        ups_r: RecvStream,
        ups_w: SendStream<Bytes>,
    ) -> ServerTaskResult<()> {
        let clt_r = H2StreamReader::new(clt_r);
        let clt_w = H2StreamWriter::new(clt_w);
        let ups_r = H2StreamReader::new(ups_r);
        let ups_w = H2StreamWriter::new(ups_w);

        self.transit_transparent(clt_r, clt_w, ups_r, ups_w).await
    }

    async fn do_block(
        &mut self,
        mut clt_w: SendStream<Bytes>,
        mut ups_w: SendStream<Bytes>,
    ) -> ServerTaskResult<()> {
        const SERVER_CLOSE_BYTES: [u8; 4] = ServerCloseFrame::encode_with_status_code(1001);
        const CLIENT_CLOSE_BYTES: [u8; 8] = ClientCloseFrame::encode_with_status_code(1001);

        let _ = ups_w.send_data(Bytes::from_static(&CLIENT_CLOSE_BYTES), true);
        let _ = clt_w.send_data(Bytes::from_static(&SERVER_CLOSE_BYTES), true);
        Err(ServerTaskError::InternalAdapterError(anyhow!(
            "websocket blocked by inspection policy"
        )))
    }

    async fn do_intercept(
        &mut self,
        clt_r: RecvStream,
        clt_w: SendStream<Bytes>,
        ups_r: RecvStream,
        ups_w: SendStream<Bytes>,
    ) -> ServerTaskResult<()> {
        let clt_r = H2StreamReader::new(clt_r);
        let clt_w = H2StreamWriter::new(clt_w);
        let ups_r = H2StreamReader::new(ups_r);
        let ups_w = H2StreamWriter::new(ups_w);

        self.transit_transparent(clt_r, clt_w, ups_r, ups_w).await
    }
}
