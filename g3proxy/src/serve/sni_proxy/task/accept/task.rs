/*
 * SPDX-License-Identifier: Apache-2.0
 * Copyright 2023-2025 ByteDance and/or its affiliates.
 */

use std::sync::Arc;

use bytes::{Buf, BytesMut};
use log::debug;
use tokio::io::{AsyncRead, AsyncReadExt, AsyncWrite};
use tokio::net::TcpStream;
use tokio::time::Instant;

use g3_daemon::stat::task::TcpStreamConnectionStats;
use g3_dpi::{Protocol, ProtocolInspectError, ProtocolInspector};
use g3_io_ext::{LimitedReader, LimitedWriter};
use g3_types::auth::FactsMatchType;
use g3_types::net::{Host, UpstreamAddr};

use super::{CommonTaskContext, SniProxyCltWrapperStats, TcpStreamTask};
use crate::audit::AuditContext;
use crate::auth::UserContext;
use crate::config::server::ServerConfig;
use crate::serve::{
    ServerStats, ServerTaskError, ServerTaskForbiddenError, ServerTaskNotes, ServerTaskResult,
};

pub(crate) struct ClientHelloAcceptTask {
    ctx: CommonTaskContext,
    audit_ctx: AuditContext,
    time_accepted: Instant,
    pre_handshake_stats: Arc<TcpStreamConnectionStats>,
}

impl ClientHelloAcceptTask {
    pub(crate) fn new(ctx: CommonTaskContext, audit_ctx: AuditContext) -> Self {
        ClientHelloAcceptTask {
            ctx,
            audit_ctx,
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
        let clt_r = LimitedReader::local_limited(
            clt_r,
            limit_config.shift_millis,
            limit_config.max_north,
            clt_r_stats,
        );
        let clt_w = LimitedWriter::local_limited(
            clt_w,
            limit_config.shift_millis,
            limit_config.max_south,
            clt_w_stats,
        );

        let client_addr = self.ctx.client_addr();
        if let Err(e) = self.run(clt_r, clt_w).await {
            debug!("Error handling client {client_addr}: {e}");
            // TODO handle client hello error
        }
    }

    fn pre_start(&self) {
        debug!(
            "new client from {} to {} server {}, using escaper {}",
            self.ctx.client_addr(),
            self.ctx.server_config.r#type(),
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
                ));
            }
        }

        let (mut upstream, protocol) = tokio::time::timeout(
            self.ctx.server_config.request_recv_timeout,
            self.inspect(&mut clt_r, &mut clt_r_buf),
        )
        .await
        .map_err(|_| {
            ServerTaskError::ClientAppTimeout("timeout to receive full client request")
        })??;

        if let Some(allowed_sites) = &self.ctx.server_config.allowed_sites {
            if let Some(site) = allowed_sites.get(upstream.host()) {
                upstream = site.redirect(&upstream);
            } else {
                // just close the connection
                return Err(ServerTaskError::ForbiddenByRule(
                    ServerTaskForbiddenError::DestDenied,
                ));
            }
        }

        let task_notes = if let Some(auth_match) = self.ctx.server_config.auth_match {
            let user_group = self
                .ctx
                .user_group
                .as_ref()
                .ok_or(ServerTaskError::ClientAuthFailed)?;

            let (user, user_type) = match auth_match {
                FactsMatchType::ClientIp => user_group
                    .get_user_by_ip(self.ctx.cc_info.client_ip())
                    .ok_or(ServerTaskError::ClientAuthFailed)?,
                FactsMatchType::ServerIp => {
                    return Err(ServerTaskError::ClientAuthFailed);
                }
                FactsMatchType::ServerName => match upstream.host() {
                    Host::Ip(ip) => user_group
                        .get_user_by_ip(*ip)
                        .ok_or(ServerTaskError::ClientAuthFailed)?,
                    Host::Domain(domain) => user_group
                        .get_user_by_domain(domain)
                        .ok_or(ServerTaskError::ClientAuthFailed)?,
                },
            };

            let user_ctx = UserContext::new(
                None,
                user,
                user_type,
                self.ctx.server_config.name(),
                self.ctx.server_stats.share_extra_tags(),
            );
            if user_ctx.check_client_addr(self.ctx.client_addr()).is_err() {
                // TODO may be attack
                return Err(ServerTaskError::ForbiddenByRule(
                    ServerTaskForbiddenError::ClientIpBlocked,
                ));
            }
            ServerTaskNotes::new(
                self.ctx.cc_info.clone(),
                Some(user_ctx),
                self.time_accepted.elapsed(),
            )
        } else {
            ServerTaskNotes::new(self.ctx.cc_info.clone(), None, self.time_accepted.elapsed())
        };

        TcpStreamTask::new(
            self.ctx,
            self.audit_ctx,
            protocol,
            upstream,
            self.pre_handshake_stats.as_ref().clone(),
            task_notes,
        )
        .into_running(clt_r, clt_r_buf, clt_w)
        .await;
        Ok(())
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
                self.ctx.server_port(),
                clt_r_buf.chunk(),
            ) {
                Ok(p) => {
                    let upstream = self.fetch_upstream(p, clt_r, clt_r_buf).await?;
                    return Ok((upstream, p));
                }
                Err(ProtocolInspectError::NeedMoreData(_)) => {
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
                super::http::parse_request(clt_r, clt_r_buf, self.ctx.server_port()).await
            }
            Protocol::TlsModern => {
                super::tls::parse_request(
                    clt_r,
                    clt_r_buf,
                    self.ctx.server_port(),
                    self.ctx.server_config.tls_max_client_hello_size,
                )
                .await
            }
            _ => Err(ServerTaskError::InvalidClientProtocol(
                "unsupported client protocol",
            )),
        }
    }
}
