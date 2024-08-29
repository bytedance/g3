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

use std::future::poll_fn;
use std::sync::Arc;

use async_recursion::async_recursion;
use bytes::Bytes;
use h2::{server::Connection, Reason};
use slog::slog_info;
use tokio::io::{AsyncRead, AsyncWrite, AsyncWriteExt};
use tokio::time::Instant;

use g3_dpi::{Protocol, ProtocolInspectPolicy};
use g3_h2::H2BodyTransfer;
use g3_io_ext::OnceBufReader;
use g3_slog_types::{LtUpstreamAddr, LtUuid};
use g3_types::net::UpstreamAddr;

#[cfg(feature = "quic")]
use crate::audit::DetourAction;
use crate::config::server::ServerConfig;
use crate::inspect::{BoxAsyncRead, BoxAsyncWrite, InterceptionError, StreamInspectContext};
use crate::serve::ServerTaskResult;

mod error;
pub(crate) use error::{H2InterceptionError, H2StreamTransferError};

mod stats;
use stats::H2ConcurrencyStats;

mod stream;

mod connect;
use connect::{H2ConnectTask, H2ExtendedConnectTask};

mod forward;
use forward::H2ForwardTask;

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
}

macro_rules! intercept_log {
    ($obj:tt, $($args:tt)+) => {
        slog_info!($obj.ctx.intercept_logger(), $($args)+;
            "intercept_type" => "H2Connection",
            "task_id" => LtUuid($obj.ctx.server_task_id()),
            "depth" => $obj.ctx.inspection_depth,
            "upstream" => LtUpstreamAddr(&$obj.upstream),
            "total_sub_task" => $obj.stats.get_total_task(),
            "alive_sub_task" => $obj.stats.get_alive_task(),
        )
    };
}

impl<SC> H2InterceptObject<SC>
where
    SC: ServerConfig + Send + Sync + 'static,
{
    pub(crate) async fn intercept(mut self) -> ServerTaskResult<()> {
        let r = match self.ctx.h2_inspect_policy() {
            ProtocolInspectPolicy::Intercept => self
                .do_intercept()
                .await
                .map_err(|e| InterceptionError::H2(e).into_server_task_error(Protocol::Http2)),
            #[cfg(feature = "quic")]
            ProtocolInspectPolicy::Detour => self.do_detour().await,
            ProtocolInspectPolicy::Bypass => self.do_bypass().await,
            ProtocolInspectPolicy::Block => self
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
            .max_frame_size(http_config.max_frame_size)
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
            .max_frame_size(http_config.max_frame_size)
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
            .max_concurrent_streams(http_config.max_concurrent_streams)
            .max_frame_size(http_config.max_frame_size)
            .max_send_buffer_size(http_config.max_send_buffer_size);
        if http_config.disable_upstream_push {
            client_builder.enable_push(false);
        }

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

        let max_concurrent_recv_streams =
            u32::try_from(h2s_connection.max_concurrent_recv_streams()).unwrap_or(u32::MAX);

        let mut server_builder = h2::server::Builder::new();
        server_builder
            .max_header_list_size(http_config.max_header_list_size)
            .max_concurrent_streams(max_concurrent_recv_streams)
            .max_frame_size(http_config.max_frame_size)
            .max_send_buffer_size(http_config.max_send_buffer_size);
        if h2s.is_extended_connect_protocol_enabled() {
            server_builder.enable_connect_protocol();
        }

        let mut h2c = match tokio::time::timeout(
            http_config.client_handshake_timeout,
            server_builder.handshake(tokio::io::join(clt_r, clt_w)),
        )
        .await
        {
            Ok(Ok(d)) => d,
            Ok(Err(e)) => return Err(H2InterceptionError::client_handshake_failed(e)),
            Err(_) => return Err(H2InterceptionError::ClientHandshakeTimeout),
        };

        // TODO spawn ping-pong

        let idle_duration = self.ctx.server_config.task_idle_check_duration();
        let mut idle_interval =
            tokio::time::interval_at(Instant::now() + idle_duration, idle_duration);
        let mut idle_count = 0;
        let max_idle_count = self.ctx.task_max_idle_count();

        loop {
            tokio::select! {
                biased;

                ups_r = &mut h2s_connection => {
                    return match ups_r {
                        Ok(_) => {
                            server_graceful_shutdown(h2c).await;

                            Ok(())
                        }
                        Err(e) => {
                            server_graceful_shutdown(h2c).await;

                            if let Some(e) = e.get_io() {
                                if e.kind() == std::io::ErrorKind::NotConnected {
                                    return Err(H2InterceptionError::UpstreamConnectionDisconnected);
                                }
                            }
                            Err(H2InterceptionError::UpstreamConnectionClosed(e))
                        }
                    };
                }
                clt_r = h2c.accept() => {
                    match clt_r {
                        Some(Ok((clt_req, clt_send_rsp))) => {
                            let h2s = h2s.clone();
                            let ctx = self.ctx.clone();
                            let stats = self.stats.clone();
                            stats.add_task();
                            tokio::spawn(async move {
                                stream::transfer(clt_req, clt_send_rsp, h2s, ctx).await;
                                stats.del_task();
                            });
                            continue;
                        }
                        Some(Err(e)) => {
                            // close all stream and wait the h2s connection to close
                            drop(h2s);
                            // TODO add timeout
                            let _ = h2s_connection.await;

                            if let Some(e) = e.get_io() {
                                if e.kind() == std::io::ErrorKind::NotConnected {
                                    return Ok(());
                                }
                            }
                            return Err(H2InterceptionError::ClientConnectionClosed(e));
                        }
                        None => {
                            // close all stream and wait the h2s connection to close
                            drop(h2s);
                            // TODO add timeout
                            let _ = h2s_connection.await;

                            return Ok(());
                        }
                    }
                }
                _ = idle_interval.tick() => {
                    if self.stats.get_alive_task() <= 0 {
                        idle_count += 1;

                        if idle_count > max_idle_count {
                            server_abrupt_shutdown(h2c, Reason::ENHANCE_YOUR_CALM).await;

                            return Err(H2InterceptionError::Idle(idle_duration, idle_count));
                        }
                    } else {
                        idle_count = 0;
                    }

                    if self.ctx.belongs_to_blocked_user() {
                        server_abrupt_shutdown(h2c, Reason::CANCEL).await;

                        return Err(H2InterceptionError::CanceledAsUserBlocked);
                    }

                    if self.ctx.server_force_quit() {
                        server_abrupt_shutdown(h2c, Reason::CANCEL).await;

                        return Err(H2InterceptionError::CanceledAsServerQuit)
                    }

                    if self.ctx.server_offline() {
                        h2c.graceful_shutdown();
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

    while let Some(r) = h2c.accept().await {
        match r {
            Ok((_req, mut send_rsp)) => {
                send_rsp.send_reset(Reason::REFUSED_STREAM);
            }
            Err(_) => break,
        }
    }
}

async fn server_abrupt_shutdown<T>(mut h2c: Connection<T, Bytes>, reason: Reason)
where
    T: AsyncRead + AsyncWrite + Unpin,
{
    h2c.abrupt_shutdown(reason);

    while let Some(r) = h2c.accept().await {
        match r {
            Ok((_req, mut send_rsp)) => {
                send_rsp.send_reset(reason);
            }
            Err(_) => break,
        }
    }
}
