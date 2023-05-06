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

use log::debug;
use tokio::io::{AsyncRead, AsyncReadExt, AsyncWrite, BufReader};
use tokio::net::TcpStream;
use tokio::time::Instant;

use g3_io_ext::{LimitedReader, LimitedWriter};
use g3_socks::{v4a, v5, SocksAuthMethod, SocksCommand, SocksVersion};
use g3_types::route::EgressPathSelection;

use super::tcp_connect::SocksProxyTcpConnectTask;
use super::udp_associate::SocksProxyUdpAssociateTask;
use super::udp_connect::SocksProxyUdpConnectTask;
use super::{CommonTaskContext, SocksProxyCltWrapperStats};
use crate::auth::{UserContext, UserGroup};
use crate::config::server::ServerConfig;
use crate::serve::{
    ServerStats, ServerTaskError, ServerTaskForbiddenError, ServerTaskNotes, ServerTaskResult,
};

pub(crate) struct SocksProxyNegotiationTask {
    pub(crate) ctx: CommonTaskContext,
    user_group: Option<Arc<UserGroup>>,
    time_accepted: Instant,
}

impl SocksProxyNegotiationTask {
    pub(crate) fn new(ctx: CommonTaskContext, user_group: Option<Arc<UserGroup>>) -> Self {
        SocksProxyNegotiationTask {
            ctx,
            user_group,
            time_accepted: Instant::now(),
        }
    }

    pub(crate) async fn into_running(self, stream: TcpStream) {
        self.pre_start();

        let (clt_r_stats, clt_w_stats) =
            SocksProxyCltWrapperStats::new_pair(&self.ctx.server_stats);
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

        let client_addr = self.ctx.tcp_client_addr;
        if let Err(e) = self.run(BufReader::new(clt_r), clt_w).await {
            debug!("Error handling client {}: {}", client_addr, e);
            // TODO handle negotiation error
        }
    }

    fn pre_start(&self) {
        debug!(
            "new client from {} to {} server {}, using escaper {}",
            self.ctx.tcp_client_addr,
            self.ctx.server_config.server_type(),
            self.ctx.server_config.name(),
            self.ctx.server_config.escaper
        );
    }

    async fn run<CDR, CDW>(
        self,
        mut clt_r: BufReader<LimitedReader<CDR>>,
        clt_w: LimitedWriter<CDW>,
    ) -> ServerTaskResult<()>
    where
        CDR: AsyncRead + Send + Sync + Unpin + 'static,
        CDW: AsyncWrite + Send + Sync + Unpin + 'static,
    {
        let timeout = self.ctx.server_config.timeout.negotiation;
        let fut = async {
            let version = clt_r
                .read_u8()
                .await
                .map_err(ServerTaskError::ClientTcpReadFailed)?;
            match version {
                0x04 => self.run_v4(clt_r, clt_w).await,
                0x05 => self.run_v5(clt_r, clt_w).await,
                _ => Err(ServerTaskError::InvalidClientProtocol(
                    "invalid socks version",
                )),
            }
        };
        match tokio::time::timeout(timeout, fut).await {
            Ok(ret) => ret,
            Err(_) => Err(ServerTaskError::ClientAppTimeout("negotiation timeout")),
        }
    }

    async fn run_v4<CDR, CDW>(
        self,
        mut clt_r: BufReader<LimitedReader<CDR>>,
        mut clt_w: LimitedWriter<CDW>,
    ) -> ServerTaskResult<()>
    where
        CDR: AsyncRead + Send + Sync + Unpin + 'static,
        CDW: AsyncWrite + Send + Sync + Unpin + 'static,
    {
        if self.user_group.is_some() {
            // socks4(a) doesn't support auth
            self.ctx.server_stats.forbidden.add_auth_failed();
            return Err(ServerTaskError::InvalidClientProtocol(
                "socks4 does not support auth",
            ));
        }

        let req = v4a::SocksV4aRequest::recv(&mut clt_r).await?;

        let task_notes = ServerTaskNotes::new(
            self.ctx.worker_id,
            self.ctx.tcp_client_addr,
            self.ctx.tcp_server_addr,
            None,
            self.time_accepted.elapsed(),
            EgressPathSelection::Default,
        );
        match req.command {
            SocksCommand::TcpConnect => {
                let task = SocksProxyTcpConnectTask::new(
                    SocksVersion::V4a,
                    self.ctx,
                    task_notes,
                    req.upstream,
                );
                task.into_running(clt_r.into_inner(), clt_w);
                Ok(())
            }
            SocksCommand::TcpBind => {
                let _ = v4a::SocksV4Reply::RequestRejectedOrFailed
                    .send(&mut clt_w)
                    .await;
                Err(ServerTaskError::UnimplementedProtocol)
            }
            _ => Err(ServerTaskError::InvalidClientProtocol(
                "invalid socks4 command",
            )),
        }
    }

    async fn run_v5<CDR, CDW>(
        self,
        mut clt_r: BufReader<LimitedReader<CDR>>,
        mut clt_w: LimitedWriter<CDW>,
    ) -> ServerTaskResult<()>
    where
        CDR: AsyncRead + Send + Sync + Unpin + 'static,
        CDW: AsyncWrite + Send + Sync + Unpin + 'static,
    {
        let client_methods = v5::auth::recv_methods_from_client(&mut clt_r).await?;
        let auth_method = if let Some(user_group) = &self.user_group {
            if client_methods.contains(&SocksAuthMethod::User) {
                SocksAuthMethod::User
            } else if user_group.allow_anonymous() {
                SocksAuthMethod::None
            } else {
                SocksAuthMethod::User
            }
        } else {
            SocksAuthMethod::None
        };
        if !client_methods.contains(&auth_method) {
            let _ =
                v5::auth::send_method_to_client(&mut clt_w, &SocksAuthMethod::NoAcceptable).await;
            self.ctx.server_stats.forbidden.add_auth_failed();
            return Err(ServerTaskError::ClientAuthFailed);
        }

        v5::auth::send_method_to_client(&mut clt_w, &auth_method)
            .await
            .map_err(ServerTaskError::ClientTcpWriteFailed)?;

        let user_ctx = match auth_method {
            SocksAuthMethod::None => {
                if let Some(user_group) = &self.user_group {
                    if let Some((user, user_type)) = user_group.get_anonymous_user() {
                        let user_ctx = UserContext::new(
                            user,
                            user_type,
                            self.ctx.server_config.name(),
                            self.ctx.server_stats.extra_tags(),
                        );
                        user_ctx.req_stats().conn_total.add_socks();
                        // TODO handle site level conn_total stats in each tasks
                        Some(user_ctx)
                    } else {
                        None
                    }
                } else {
                    None
                }
            }
            SocksAuthMethod::User => {
                if let Some(user_group) = &self.user_group {
                    let (username, password) = v5::auth::recv_user_from_client(&mut clt_r).await?;
                    if let Some((user, user_type)) = user_group.get_user(username.as_original()) {
                        let user_ctx = UserContext::new(
                            user,
                            user_type,
                            self.ctx.server_config.name(),
                            self.ctx.server_stats.extra_tags(),
                        );
                        match user_ctx.check_password(password.as_original()) {
                            Ok(_) => {
                                user_ctx.req_stats().conn_total.add_socks();
                                // TODO handle site level conn_total stats in each tasks
                                v5::auth::send_user_auth_success(&mut clt_w)
                                    .await
                                    .map_err(ServerTaskError::ClientTcpWriteFailed)?;
                            }
                            Err(e) => {
                                return if let Some(duration) = e.blocked_delay() {
                                    self.ctx.server_stats.forbidden.add_user_blocked();
                                    tokio::time::sleep(duration).await;
                                    let _ = v5::Socks5Reply::ForbiddenByRule.send(&mut clt_w).await;
                                    Err(ServerTaskError::ForbiddenByRule(
                                        ServerTaskForbiddenError::UserBlocked,
                                    ))
                                } else {
                                    self.ctx.server_stats.forbidden.add_auth_failed();
                                    let _ = v5::auth::send_user_auth_failure(&mut clt_w).await;
                                    Err(ServerTaskError::ClientAuthFailed)
                                };
                            }
                        }
                        Some(user_ctx)
                    } else {
                        self.ctx.server_stats.forbidden.add_auth_failed();
                        let _ = v5::auth::send_user_auth_failure(&mut clt_w).await;
                        return Err(ServerTaskError::ClientAuthFailed);
                    }
                } else {
                    unreachable!()
                }
            }
            _ => return Err(ServerTaskError::UnimplementedProtocol),
        };

        let req = v5::Socks5Request::recv(&mut clt_r).await?;

        let task_notes = ServerTaskNotes::new(
            self.ctx.worker_id,
            self.ctx.tcp_client_addr,
            self.ctx.tcp_server_addr,
            user_ctx,
            self.time_accepted.elapsed(),
            EgressPathSelection::Default,
        );
        match req.command {
            SocksCommand::TcpConnect => {
                let task = SocksProxyTcpConnectTask::new(
                    SocksVersion::V5,
                    self.ctx,
                    task_notes,
                    req.upstream,
                );
                task.into_running(clt_r.into_inner(), clt_w);
                Ok(())
            }
            SocksCommand::UdpAssociate => {
                let udp_check_addr = match req.udp_peer_addr() {
                    Ok(addr) => addr,
                    Err(e) => {
                        let _ = v5::Socks5Reply::AddressTypeNotSupported
                            .send(&mut clt_w)
                            .await;
                        return Err(e.into());
                    }
                };

                let use_udp_associate = self.ctx.server_config.use_udp_associate
                    || task_notes
                        .user_ctx()
                        .map(|uc| uc.user().config.socks_use_udp_associate)
                        .unwrap_or(false);
                if use_udp_associate {
                    let task =
                        SocksProxyUdpAssociateTask::new(self.ctx, task_notes, udp_check_addr);
                    task.into_running(clt_r.into_inner(), clt_w);
                    Ok(())
                } else {
                    let task = SocksProxyUdpConnectTask::new(self.ctx, task_notes, udp_check_addr);
                    task.into_running(clt_r.into_inner(), clt_w);
                    Ok(())
                }
            }
            SocksCommand::TcpBind => {
                let _ = v5::Socks5Reply::CommandNotSupported.send(&mut clt_w).await;
                Err(ServerTaskError::UnimplementedProtocol)
            }
        }
    }
}
