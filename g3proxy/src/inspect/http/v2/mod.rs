/*
 * SPDX-License-Identifier: Apache-2.0
 * Copyright 2023-2025 ByteDance and/or its affiliates.
 */

use std::future::poll_fn;
use std::sync::Arc;
use std::time::Duration;

use async_recursion::async_recursion;
use bytes::Bytes;
use h2::{Reason, server::Connection};
use tokio::io::{AsyncRead, AsyncWrite, AsyncWriteExt};
use tokio::sync::oneshot;

use g3_daemon::server::ServerQuitPolicy;
use g3_dpi::{Protocol, ProtocolInspectAction};
use g3_h2::H2BodyTransfer;
use g3_io_ext::{IdleInterval, OnceBufReader, StreamCopyConfig};
use g3_slog_types::{LtUpstreamAddr, LtUuid};
use g3_types::net::UpstreamAddr;

#[cfg(feature = "quic")]
use crate::audit::DetourAction;
use crate::auth::User;
use crate::config::server::ServerConfig;
use crate::inspect::{
    BoxAsyncRead, BoxAsyncWrite, InterceptionError, StreamInspectContext, StreamTransitTask,
};
use crate::log::task::TaskEvent;
use crate::serve::ServerTaskResult;

mod error;
pub(crate) use error::{H2InterceptionError, H2StreamTransferError};

mod stats;
use stats::H2ConcurrencyStats;

mod stream;

mod ping;
use ping::H2PingTask;

mod connect;
use connect::{H2ConnectTask, H2ExtendedConnectTask};

mod forward;
use forward::H2ForwardTask;

macro_rules! intercept_log {
    ($obj:tt, $($args:tt)+) => {
        if let Some(logger) = $obj.ctx.intercept_logger() {
            slog::info!(logger, $($args)+;
                "intercept_type" => "H2Connection",
                "task_id" => LtUuid($obj.ctx.server_task_id()),
                "depth" => $obj.ctx.inspection_depth,
                "upstream" => LtUpstreamAddr(&$obj.upstream),
                "total_sub_task" => $obj.stats.get_total_task(),
                "alive_sub_task" => $obj.stats.get_alive_task(),
            );
        }
    };
}

struct H2InterceptIo {
    clt_r: OnceBufReader<BoxAsyncRead>,
    clt_w: BoxAsyncWrite,
    ups_r: BoxAsyncRead,
    ups_w: BoxAsyncWrite,
}

pub(crate) struct H2InterceptObject<SC: ServerConfig> {
    io: Option<H2InterceptIo>,
    ctx: StreamInspectContext<SC>,
    stats: Arc<H2ConcurrencyStats>,
    upstream: UpstreamAddr,
}

impl<SC: ServerConfig> H2InterceptObject<SC> {
    pub(crate) fn new(ctx: StreamInspectContext<SC>, upstream: UpstreamAddr) -> Self {
        let stats = Arc::new(H2ConcurrencyStats::default());
        H2InterceptObject {
            io: None,
            ctx,
            stats,
            upstream,
        }
    }

    pub(crate) fn set_io(
        &mut self,
        clt_r: OnceBufReader<BoxAsyncRead>,
        clt_w: BoxAsyncWrite,
        ups_r: BoxAsyncRead,
        ups_w: BoxAsyncWrite,
    ) {
        let io = H2InterceptIo {
            clt_r,
            clt_w,
            ups_r,
            ups_w,
        };
        self.io = Some(io);
    }

    fn log_partial_shutdown(&self, task_event: TaskEvent) {
        if let Some(logger) = self.ctx.intercept_logger() {
            slog::info!(logger, "";
                "intercept_type" => "H2Connection",
                "task_id" => LtUuid(self.ctx.server_task_id()),
                "task_event" => task_event.as_str(),
                "depth" => self.ctx.inspection_depth,
                "upstream" => LtUpstreamAddr(&self.upstream),
            );
        }
    }
}

impl<SC: ServerConfig> StreamTransitTask for H2InterceptObject<SC> {
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

impl<SC> H2InterceptObject<SC>
where
    SC: ServerConfig + Send + Sync + 'static,
{
    pub(crate) async fn intercept(mut self) -> ServerTaskResult<()> {
        let r = match self.ctx.h2_inspect_action(self.upstream.host()) {
            ProtocolInspectAction::Intercept => self
                .do_intercept()
                .await
                .map_err(|e| InterceptionError::H2(e).into_server_task_error(Protocol::Http2)),
            #[cfg(feature = "quic")]
            ProtocolInspectAction::Detour => self.do_detour().await,
            ProtocolInspectAction::Bypass => self.do_bypass().await,
            ProtocolInspectAction::Block => self
                .do_block()
                .await
                .map_err(|e| InterceptionError::H2(e).into_server_task_error(Protocol::Http2)),
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
        use crate::serve::ServerTaskError;

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
            Protocol::Http2,
        );

        match detour_ctx.check_detour_action(&mut detour_stream).await {
            Ok(DetourAction::Continue) => {
                let H2InterceptIo {
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
                self.do_block()
                    .await
                    .map_err(|e| InterceptionError::H2(e).into_server_task_error(Protocol::Http2))
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
        let H2InterceptIo {
            clt_r,
            clt_w,
            ups_r: _,
            mut ups_w,
        } = self.io.take().unwrap();

        tokio::spawn(async move {
            let _ = ups_w.shutdown().await;
        });

        let http_config = self.ctx.h2_interception();
        let mut server_builder = h2::server::Builder::new();
        server_builder
            .max_header_list_size(http_config.max_header_list_size)
            .max_concurrent_streams(1)
            .max_frame_size(http_config.max_frame_size())
            .max_send_buffer_size(http_config.max_send_buffer_size);

        match tokio::time::timeout(
            http_config.client_handshake_timeout,
            server_builder.handshake::<_, Bytes>(tokio::io::join(clt_r, clt_w)),
        )
        .await
        {
            Ok(Ok(mut h2c)) => {
                h2c.abrupt_shutdown(Reason::INTERNAL_ERROR);
                // TODO add timeout
                let _ = poll_fn(|ctx| h2c.poll_closed(ctx)).await;
            }
            Ok(Err(_)) => {}
            Err(_) => {}
        };
    }

    async fn do_bypass(&mut self) -> ServerTaskResult<()> {
        let H2InterceptIo {
            clt_r,
            clt_w,
            ups_r,
            ups_w,
        } = self.io.take().unwrap();

        self.transit_transparent(clt_r, clt_w, ups_r, ups_w).await
    }

    async fn do_block(&mut self) -> Result<(), H2InterceptionError> {
        let H2InterceptIo {
            clt_r,
            clt_w,
            ups_r: _,
            mut ups_w,
        } = self.io.take().unwrap();

        tokio::spawn(async move {
            let _ = ups_w.shutdown().await;
        });

        let http_config = self.ctx.h2_interception();
        let mut server_builder = h2::server::Builder::new();
        server_builder
            .max_header_list_size(http_config.max_header_list_size)
            .max_concurrent_streams(1)
            .max_frame_size(http_config.max_frame_size())
            .max_send_buffer_size(http_config.max_send_buffer_size);

        let mut h2c = match tokio::time::timeout(
            http_config.client_handshake_timeout,
            server_builder.handshake::<_, Bytes>(tokio::io::join(clt_r, clt_w)),
        )
        .await
        {
            Ok(Ok(d)) => d,
            Ok(Err(e)) => return Err(H2InterceptionError::client_handshake_failed(e)),
            Err(_) => return Err(H2InterceptionError::ClientHandshakeTimeout),
        };

        h2c.abrupt_shutdown(Reason::HTTP_1_1_REQUIRED);

        // TODO add timeout
        let _ = poll_fn(|ctx| h2c.poll_closed(ctx)).await;

        Err(H2InterceptionError::ClientConnectionBlocked)
    }

    #[async_recursion]
    async fn do_intercept(&mut self) -> Result<(), H2InterceptionError> {
        let H2InterceptIo {
            clt_r,
            clt_w,
            ups_r,
            ups_w,
        } = self.io.take().unwrap();

        let http_config = self.ctx.h2_interception();
        let mut client_builder = h2::client::Builder::new();
        client_builder
            .enable_push(false) // server push is deprecated by chrome and nginx
            .max_header_list_size(http_config.max_header_list_size)
            .max_concurrent_streams(0)
            .max_frame_size(http_config.max_frame_size())
            .max_send_buffer_size(http_config.max_send_buffer_size)
            .initial_window_size(http_config.stream_window_size())
            .initial_connection_window_size(http_config.connection_window_size());

        let (h2s, mut h2s_connection) = match tokio::time::timeout(
            http_config.upstream_handshake_timeout,
            client_builder.handshake(tokio::io::join(ups_r, ups_w)),
        )
        .await
        {
            Ok(Ok(d)) => d,
            Ok(Err(e)) => return Err(H2InterceptionError::upstream_handshake_failed(e)),
            Err(_) => return Err(H2InterceptionError::UpstreamHandshakeTimeout),
        };

        let (ping_quit_sender, ping_quit_receiver) = oneshot::channel();
        if let Some(ping) = h2s_connection.ping_pong() {
            let ping_task =
                H2PingTask::new(self.ctx.clone(), self.stats.clone(), self.upstream.clone());
            tokio::spawn(ping_task.into_running(ping, ping_quit_receiver));
        }
        let max_concurrent_send_streams =
            u32::try_from(h2s_connection.max_concurrent_send_streams())
                .unwrap_or(u32::MAX)
                .min(http_config.max_concurrent_streams);
        let (ups_close_sender, mut ups_close_receiver) = oneshot::channel();
        tokio::spawn(async move {
            if let Err(e) = h2s_connection.await {
                let _ = ups_close_sender.send(e);
            }
        });

        let mut server_builder = h2::server::Builder::new();
        server_builder
            .max_header_list_size(http_config.max_header_list_size)
            .max_concurrent_streams(max_concurrent_send_streams)
            .max_frame_size(http_config.max_frame_size())
            .max_send_buffer_size(http_config.max_send_buffer_size)
            .initial_window_size(http_config.stream_window_size())
            .initial_connection_window_size(http_config.connection_window_size());
        if h2s.is_extended_connect_protocol_enabled() {
            server_builder.enable_connect_protocol();
        }

        let mut h2c_connection = match tokio::time::timeout(
            http_config.client_handshake_timeout,
            server_builder.handshake(tokio::io::join(clt_r, clt_w)),
        )
        .await
        {
            Ok(Ok(d)) => d,
            Ok(Err(e)) => return Err(H2InterceptionError::client_handshake_failed(e)),
            Err(_) => return Err(H2InterceptionError::ClientHandshakeTimeout),
        };

        let mut idle_interval = self.ctx.idle_wheel.register();
        let mut idle_count = 0;

        loop {
            tokio::select! {
                biased;

                ups_r = &mut ups_close_receiver => {
                    let _ = ping_quit_sender.send(());
                    return match ups_r {
                        Ok(e) => {
                            // upstream connection error
                            server_graceful_shutdown(h2c_connection).await;
                            if let Some(e) = e.get_io()
                                && e.kind() == std::io::ErrorKind::NotConnected {
                                    return Err(H2InterceptionError::UpstreamConnectionDisconnected);
                                }
                            Err(H2InterceptionError::UpstreamConnectionClosed(e))
                        }
                        Err(_) => {
                            // upstream connection closed
                            self.log_upstream_shutdown();
                            server_graceful_shutdown(h2c_connection).await;
                            Ok(())
                        }
                    };
                }
                clt_r = h2c_connection.accept() => {
                    match clt_r {
                        Some(Ok((clt_req, clt_send_rsp))) => {
                            let h2s = h2s.clone();
                            let ctx = self.ctx.clone();
                            let stats = self.stats.clone();
                            idle_count = 0;
                            stats.add_task();
                            tokio::spawn(async move {
                                stream::transfer(clt_req, clt_send_rsp, h2s, ctx).await;
                                stats.del_task();
                            });
                            continue;
                        }
                        Some(Err(e)) => {
                            // close all stream and let the h2s connection to close
                            drop(h2s);
                            let _ = ping_quit_sender.send(());
                            // h2c_connection.poll_closed() has already been called in accept()

                            if let Some(e) = e.get_io()
                                && e.kind() == std::io::ErrorKind::NotConnected {
                                    return Ok(());
                                }
                            return Err(H2InterceptionError::ClientConnectionClosed(e));
                        }
                        None => {
                            // close all stream and let the h2s connection to close
                            drop(h2s);
                            let _ = ping_quit_sender.send(());
                            self.log_client_shutdown();
                            let _ = poll_fn(|cx| h2c_connection.poll_closed(cx)).await;
                            return Ok(());
                        }
                    }
                }
                n = idle_interval.tick() => {
                    if self.stats.get_alive_task() <= 0 {
                        idle_count += n;

                        if idle_count > self.ctx.max_idle_count {
                            let _ = ping_quit_sender.send(());
                            server_abrupt_shutdown(h2c_connection, Reason::NO_ERROR).await;

                            return Err(H2InterceptionError::Idle(idle_interval.period(), idle_count));
                        }
                    } else {
                        idle_count = 0;
                    }

                    if self.ctx.belongs_to_blocked_user() {
                        let _ = ping_quit_sender.send(());
                        server_abrupt_shutdown(h2c_connection, Reason::ENHANCE_YOUR_CALM).await;

                        return Err(H2InterceptionError::CanceledAsUserBlocked);
                    }

                    if self.ctx.server_force_quit() {
                        let _ = ping_quit_sender.send(());
                        server_graceful_shutdown(h2c_connection).await;

                        return Err(H2InterceptionError::CanceledAsServerQuit)
                    }

                    if self.ctx.server_offline() {
                        h2c_connection.graceful_shutdown();
                    }
                }
            }
        }
    }
}

async fn server_graceful_shutdown<T>(mut h2c: Connection<T, Bytes>)
where
    T: AsyncRead + AsyncWrite + Unpin,
{
    h2c.graceful_shutdown();
    server_refuse_until_closed(h2c).await;
}

async fn server_abrupt_shutdown<T>(mut h2c: Connection<T, Bytes>, reason: Reason)
where
    T: AsyncRead + AsyncWrite + Unpin,
{
    h2c.abrupt_shutdown(reason);
    server_refuse_until_closed(h2c).await;
}

async fn server_refuse_until_closed<T>(mut h2c: Connection<T, Bytes>)
where
    T: AsyncRead + AsyncWrite + Unpin,
{
    while let Some(r) = h2c.accept().await {
        match r {
            Ok((_req, mut send_rsp)) => {
                send_rsp.send_reset(Reason::REFUSED_STREAM);
            }
            Err(_) => return,
        }
    }

    let _ = poll_fn(|cx| h2c.poll_closed(cx)).await;
}
