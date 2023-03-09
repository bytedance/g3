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

use bytes::{Buf, BytesMut};
use log::debug;
use tokio::io::{AsyncRead, AsyncReadExt, AsyncWrite};
use tokio::net::TcpStream;
use tokio::time::Instant;

use g3_daemon::stat::task::TcpStreamConnectionStats;
use g3_dpi::{Protocol, ProtocolInspector};
use g3_io_ext::{LimitedReader, LimitedWriter};
use g3_types::net::UpstreamAddr;

use super::{CommonTaskContext, SniProxyCltWrapperStats, TcpStreamTask};
use crate::config::server::ServerConfig;
use crate::serve::{ServerTaskError, ServerTaskForbiddenError, ServerTaskResult};

pub(crate) struct ClientHelloAcceptTask {
    ctx: CommonTaskContext,
    time_accepted: Instant,
    pre_handshake_stats: Arc<TcpStreamConnectionStats>,
}

impl ClientHelloAcceptTask {
    pub(crate) fn new(ctx: CommonTaskContext) -> Self {
        ClientHelloAcceptTask {
            ctx,
            time_accepted: Instant::now(),
            pre_handshake_stats: Arc::new(TcpStreamConnectionStats::default()),
        }
    }

    pub(crate) async fn into_running(self, stream: TcpStream) {
        self.pre_start();

        let (clt_r_stats, clt_w_stats) =
            SniProxyCltWrapperStats::new_pair(&self.ctx.server_stats, &self.pre_handshake_stats);
        let limit_config = &self.ctx.server_config.tcp_sock_speed_limit;
        let (clt_r, clt_w) = stream.into_split();
        let clt_r = LimitedReader::new(
            clt_r,
            limit_config.shift_millis,
            limit_config.max_north,
            clt_r_stats,
        );
        let clt_w = LimitedWriter::new(
            clt_w,
            limit_config.shift_millis,
            limit_config.max_south,
            clt_w_stats,
        );

        let client_addr = self.ctx.client_addr;
        if let Err(e) = self.run(clt_r, clt_w).await {
            debug!("Error handling client {}: {}", client_addr, e);
            // TODO handle client hello error
        }
    }

    fn pre_start(&self) {
        debug!(
            "new client from {} to {} server {}, using escaper {}",
            self.ctx.client_addr,
            self.ctx.server_config.server_type(),
            self.ctx.server_config.name(),
            self.ctx.server_config.escaper
        );
    }

    async fn run<CDR, CDW>(
        self,
        mut clt_r: LimitedReader<CDR>,
        clt_w: LimitedWriter<CDW>,
    ) -> ServerTaskResult<()>
    where
        CDR: AsyncRead + Send + Sync + Unpin + 'static,
        CDW: AsyncWrite + Send + Sync + Unpin + 'static,
    {
        let inspect_buffer_size = self
            .ctx
            .server_config
            .protocol_inspection
            .data0_buffer_size();
        let mut clt_r_buf = BytesMut::with_capacity(inspect_buffer_size);

        match tokio::time::timeout(
            self.ctx.server_config.request_wait_timeout,
            clt_r.read_buf(&mut clt_r_buf),
        )
        .await
        {
            Ok(Ok(_)) => {}
            Ok(Err(e)) => return Err(ServerTaskError::ClientTcpReadFailed(e)),
            Err(_) => {
                return Err(ServerTaskError::ClientAppTimeout(
                    "timeout to wait client request",
                ))
            }
        }

        let (upstream, protocol) = tokio::time::timeout(
            self.ctx.server_config.request_recv_timeout,
            self.inspect(&mut clt_r, &mut clt_r_buf),
        )
        .await
        .map_err(|_| {
            ServerTaskError::ClientAppTimeout("timeout to receive full client request")
        })??;

        if let Some(allowed_sites) = &self.ctx.server_config.allowed_sites {
            if let Some(site) = allowed_sites.get(upstream.host()) {
                let final_upstream = site.redirect(&upstream);
                TcpStreamTask::new(
                    self.ctx,
                    protocol,
                    final_upstream,
                    self.time_accepted.elapsed(),
                    *self.pre_handshake_stats,
                )
                .into_running(clt_r, clt_r_buf, clt_w)
                .await;
                Ok(())
            } else {
                // just close the connection
                Err(ServerTaskError::ForbiddenByRule(
                    ServerTaskForbiddenError::DestDenied,
                ))
            }
        } else {
            TcpStreamTask::new(
                self.ctx,
                protocol,
                upstream,
                self.time_accepted.elapsed(),
                *self.pre_handshake_stats,
            )
            .into_running(clt_r, clt_r_buf, clt_w)
            .await;
            Ok(())
        }
    }

    async fn inspect<CDR>(
        &self,
        clt_r: &mut LimitedReader<CDR>,
        clt_r_buf: &mut BytesMut,
    ) -> ServerTaskResult<(UpstreamAddr, Protocol)>
    where
        CDR: AsyncRead + Send + Sync + Unpin + 'static,
    {
        let mut inspector = ProtocolInspector::new(
            self.ctx.server_tcp_portmap.clone(),
            self.ctx.client_tcp_portmap.clone(),
        );

        loop {
            match inspector.check_client_initial_data(
                &self.ctx.server_config.protocol_inspection,
                self.ctx.server_addr.port(),
                clt_r_buf.chunk(),
            ) {
                Ok(p) => {
                    let upstream = self.fetch_upstream(p, clt_r, clt_r_buf).await?;
                    return Ok((upstream, p));
                }
                Err(_) => {
                    if clt_r_buf.remaining() == 0 {
                        return Err(ServerTaskError::InvalidClientProtocol(
                            "unable to detect client protocol",
                        ));
                    }
                    match clt_r.read_buf(clt_r_buf).await {
                        Ok(0) => return Err(ServerTaskError::ClosedByClient),
                        Ok(_) => {}
                        Err(e) => return Err(ServerTaskError::ClientTcpReadFailed(e)),
                    }
                }
            }
        }
    }

    async fn fetch_upstream<CDR>(
        &self,
        protocol: Protocol,
        clt_r: &mut LimitedReader<CDR>,
        clt_r_buf: &mut BytesMut,
    ) -> ServerTaskResult<UpstreamAddr>
    where
        CDR: AsyncRead + Send + Sync + Unpin + 'static,
    {
        match protocol {
            Protocol::Http1 => {
                super::http::parse_request(clt_r, clt_r_buf, self.ctx.server_addr.port()).await
            }
            Protocol::TlsModern => {
                super::tls::parse_request(clt_r, clt_r_buf, self.ctx.server_addr.port()).await
            }
            _ => Err(ServerTaskError::InvalidClientProtocol(
                "unsupported client protocol",
            )),
        }
    }
}
