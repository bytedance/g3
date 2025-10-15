/*
 * SPDX-License-Identifier: Apache-2.0
 * Copyright 2023-2025 ByteDance and/or its affiliates.
 */

use std::sync::Arc;

use log::debug;
use tokio::io::{AsyncRead, AsyncReadExt, AsyncWrite, BufReader};
use tokio::time::Instant;

use g3_io_ext::{AsyncStream, LimitedReader, LimitedWriter};
use g3_socks::{SocksAuthMethod, SocksCommand, SocksVersion, v4a, v5};

use super::tcp_connect::SocksProxyTcpConnectTask;
use super::udp_associate::SocksProxyUdpAssociateTask;
use super::udp_connect::SocksProxyUdpConnectTask;
use super::{CommonTaskContext, SocksProxyCltWrapperStats};
use crate::audit::AuditContext;
use crate::auth::{UserContext, UserGroup};
use crate::config::server::ServerConfig;
use crate::escape::EgressPathSelection;
use crate::serve::{
    ServerStats, ServerTaskError, ServerTaskForbiddenError, ServerTaskNotes, ServerTaskResult,
};

pub(crate) struct SocksProxyNegotiationTask {
    pub(crate) ctx: CommonTaskContext,
    audit_ctx: AuditContext,
    user_group: Option<Arc<UserGroup>>,
    time_accepted: Instant,
}

impl SocksProxyNegotiationTask {
    pub(crate) fn new(
        ctx: CommonTaskContext,
        audit_ctx: AuditContext,
        user_group: Option<Arc<UserGroup>>,
    ) -> Self {
        SocksProxyNegotiationTask {
            ctx,
            audit_ctx,
            user_group,
            time_accepted: Instant::now(),
        }
    }

    pub(crate) async fn into_running<S>(self, stream: S)
    where
        S: AsyncStream,
        S::R: AsyncRead + Send + Sync + Unpin + 'static,
        S::W: AsyncWrite + Send + Sync + Unpin + 'static,
    {
        self.pre_start();

        let (clt_r_stats, clt_w_stats) =
            SocksProxyCltWrapperStats::new_pair(&self.ctx.server_stats);
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
        if let Err(e) = self.run(BufReader::new(clt_r), clt_w).await {
            debug!("Error handling client {client_addr}: {e}");
            // TODO handle negotiation error
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
        if let Some(user_group) = &self.user_group
            && !user_group.allow_anonymous(self.ctx.client_addr())
        {
            // socks4(a) doesn't support auth
            self.ctx.server_stats.forbidden.add_auth_failed();
            return Err(ServerTaskError::InvalidClientProtocol(
                "socks4 does not support auth",
            ));
        };

        let req = v4a::SocksV4aRequest::recv(&mut clt_r).await?;

        let user_ctx = self.user_group.map(|user_group| {
            let (user, user_type) = user_group.get_anonymous_user().unwrap();
            let user_ctx = UserContext::new(
                None,
                user,
                user_type,
                self.ctx.server_config.name(),
                self.ctx.server_stats.share_extra_tags(),
            );
            // no need to check user level client addr ACL again here
            user_ctx.req_stats().conn_total.add_socks();
            user_ctx
        });

        let task_notes = ServerTaskNotes::new(
            self.ctx.cc_info.clone(),
            user_ctx,
            self.time_accepted.elapsed(),
        );
        match req.command {
            SocksCommand::TcpConnect => {
                let task = SocksProxyTcpConnectTask::new(
                    SocksVersion::V4a,
                    self.ctx,
                    task_notes,
                    req.upstream,
                    self.audit_ctx,
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
            } else if user_group.allow_anonymous(self.ctx.client_addr()) {
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

        let mut path_selection: Option<EgressPathSelection> = None;
        let user_ctx = match auth_method {
            SocksAuthMethod::None => {
                if let Some(user_group) = &self.user_group {
                    let (user, user_type) = user_group.get_anonymous_user().unwrap();
                    let user_ctx = UserContext::new(
                        None,
                        user,
                        user_type,
                        self.ctx.server_config.name(),
                        self.ctx.server_stats.share_extra_tags(),
                    );
                    // no need to check user level client addr ACL again here
                    user_ctx.req_stats().conn_total.add_socks();
                    Some(user_ctx)
                } else {
                    None
                }
            }
            SocksAuthMethod::User => {
                if let Some(user_group) = &self.user_group {
                    let (username, password) = v5::auth::recv_user_from_client(&mut clt_r).await?;

                    let base_username;
                    if let Some(cfg) = &self.ctx.server_config.username_params {
                        match self.get_egress_path_selection(username.as_original()) {
                            Ok(Some(path)) => path_selection = Some(path),
                            Ok(None) => {}
                            Err(_) => {
                                self.ctx.server_stats.forbidden.add_dest_denied();
                                let _ = v5::Socks5Reply::ForbiddenByRule.send(&mut clt_w).await;
                                return Err(ServerTaskError::ForbiddenByRule(
                                    ServerTaskForbiddenError::DestDenied,
                                ));
                            }
                        }

                        base_username = cfg.real_username(username.as_original());
                    } else {
                        base_username = username.as_original();
                    }

                    match user_group.check_user_with_password(
                        base_username,
                        &password,
                        self.ctx.server_config.name(),
                        self.ctx.server_stats.share_extra_tags(),
                    ) {
                        Ok(user_ctx) => {
                            if user_ctx.check_client_addr(self.ctx.client_addr()).is_err() {
                                self.ctx.server_stats.forbidden.add_auth_failed();
                                let _ = v5::auth::send_user_auth_failure(&mut clt_w).await;
                                return Err(ServerTaskError::ClientAuthFailed);
                            }
                            user_ctx.req_stats().conn_total.add_socks();
                            v5::auth::send_user_auth_success(&mut clt_w)
                                .await
                                .map_err(ServerTaskError::ClientTcpWriteFailed)?;
                            Some(user_ctx)
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
                } else {
                    unreachable!()
                }
            }
            _ => return Err(ServerTaskError::UnimplementedProtocol),
        };

        let req = v5::Socks5Request::recv(&mut clt_r).await?;

        let task_notes = ServerTaskNotes::with_path_selection(
            self.ctx.cc_info.clone(),
            user_ctx,
            self.time_accepted.elapsed(),
            path_selection,
        );
        match req.command {
            SocksCommand::TcpConnect => {
                let task = SocksProxyTcpConnectTask::new(
                    SocksVersion::V5,
                    self.ctx,
                    task_notes,
                    req.upstream,
                    self.audit_ctx,
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
                        .map(|uc| uc.user_config().socks_use_udp_associate)
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

    fn get_egress_path_selection(&self, raw_name: &str) -> Result<Option<EgressPathSelection>, ()> {
        let mut egress_path = EgressPathSelection::default();

        if let Some(name_params) = &self.ctx.server_config.username_params {
            match name_params.parse_egress_upstream_socks5(raw_name) {
                Ok(Some(ups)) => {
                    debug!(
                        "[{}] socks username params -> next proxy {}",
                        self.ctx.server_config.name(),
                        ups.addr
                    );
                    egress_path.set_upstream(self.ctx.escaper.name().clone(), ups);
                }
                Ok(None) => {}
                Err(e) => {
                    debug!("failed to get upstream addr from username: {e}");
                    return Err(());
                }
            }
        }

        if egress_path.is_empty() {
            Ok(None)
        } else {
            Ok(Some(egress_path))
        }
    }
}
