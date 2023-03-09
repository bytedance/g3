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

use std::net::{IpAddr, SocketAddr};
use std::os::unix::prelude::*;
use std::sync::Arc;

use rand::seq::SliceRandom;
use slog::Logger;
use tokio::net::UdpSocket;

use g3_types::acl::{AclAction, AclNetworkRule};
use g3_types::acl_set::AclDstHostRuleSet;
use g3_types::net::UpstreamAddr;

use super::{SocksProxyServerConfig, SocksProxyServerStats};
use crate::audit::AuditHandle;
use crate::escape::ArcEscaper;
use crate::serve::{ServerQuitPolicy, ServerTaskError, ServerTaskNotes, ServerTaskResult};

#[derive(Clone)]
pub(crate) struct CommonTaskContext {
    pub(crate) server_config: Arc<SocksProxyServerConfig>,
    pub(crate) server_stats: Arc<SocksProxyServerStats>,
    pub(crate) server_quit_policy: Arc<ServerQuitPolicy>,
    pub(crate) escaper: ArcEscaper,
    pub(crate) audit_handle: Option<Arc<AuditHandle>>,
    pub(crate) ingress_net_filter: Option<Arc<AclNetworkRule>>,
    pub(crate) dst_host_filter: Option<Arc<AclDstHostRuleSet>>,
    pub(crate) tcp_server_addr: SocketAddr,
    pub(crate) tcp_client_addr: SocketAddr,
    pub(crate) task_logger: Logger,
    pub(crate) tcp_client_socket: RawFd,
    pub(crate) worker_id: Option<usize>,
}

impl CommonTaskContext {
    pub(super) fn check_upstream(&self, upstream: &UpstreamAddr) -> AclAction {
        let mut default_action = if upstream.is_empty() {
            AclAction::Forbid
        } else {
            AclAction::Permit
        };

        if let Some(filter) = &self.server_config.dst_port_filter {
            let port = upstream.port();
            let (found, action) = filter.check_port(&port);
            if found && action.forbid_early() {
                return action;
            };
            default_action = default_action.restrict(action);
        }

        if let Some(filter) = &self.dst_host_filter {
            let (found, action) = filter.check(upstream.host());
            if found && action.forbid_early() {
                return action;
            }
            default_action = default_action.restrict(action);
        }

        default_action
    }

    fn select_bind_ip(&self, ref_ip: IpAddr) -> Option<IpAddr> {
        match ref_ip {
            IpAddr::V4(_) => self
                .server_config
                .udp_bind4
                .choose(&mut rand::thread_rng())
                .copied(),
            IpAddr::V6(_) => self
                .server_config
                .udp_bind6
                .choose(&mut rand::thread_rng())
                .copied(),
        }
    }

    pub(super) fn select_udp_bind_ip(
        &self,
        udp_client_addr: Option<SocketAddr>,
    ) -> ServerTaskResult<IpAddr> {
        if let Some(addr) = udp_client_addr {
            let ref_ip = addr.ip();
            // this will allow different tcp and udp client socket families if we have set the same
            // family ip for udp bind
            if let Some(ip) = self.select_bind_ip(ref_ip) {
                return Ok(ip);
            }

            if matches!(
                (ref_ip, self.tcp_server_addr.ip()),
                (IpAddr::V4(_), IpAddr::V4(_)) | (IpAddr::V6(_), IpAddr::V6(_))
            ) {
                Ok(self.tcp_server_addr.ip())
            } else {
                Err(ServerTaskError::InvalidClientProtocol(
                    "unsupported client udp socket family",
                ))
            }
        } else {
            let ref_ip = self.tcp_server_addr.ip();
            Ok(self.select_bind_ip(ref_ip).unwrap_or(ref_ip))
        }
    }

    pub(super) async fn setup_udp_listen(
        &self,
        udp_client_addr: Option<SocketAddr>,
        task_notes: &ServerTaskNotes,
    ) -> ServerTaskResult<(SocketAddr, UdpSocket)> {
        let udp_bind_ip = self.select_udp_bind_ip(udp_client_addr)?;

        let misc_opts = if let Some(user_ctx) = task_notes.user_ctx() {
            user_ctx
                .user()
                .config
                .udp_client_misc_opts(&self.server_config.udp_misc_opts)
        } else {
            self.server_config.udp_misc_opts
        };

        let (clt_socket, listen_addr) =
            if let Some(port_range) = self.server_config.udp_bind_port_range {
                g3_socket::udp::new_std_in_range_bind_connect(
                    udp_bind_ip,
                    port_range,
                    self.server_config.udp_socket_buffer,
                    &misc_opts,
                )
                .map_err(|_| {
                    ServerTaskError::InternalServerError(
                        "setup udp listen socket with ranged port failed",
                    )
                })?
            } else {
                g3_socket::udp::new_std_bind_connect(
                    Some(udp_bind_ip),
                    self.server_config.udp_socket_buffer,
                    &misc_opts,
                )
                .map_err(|_| {
                    ServerTaskError::InternalServerError(
                        "setup udp listen socket with random port failed",
                    )
                })?
            };

        let socket = UdpSocket::from_std(clt_socket).map_err(|_| {
            ServerTaskError::InternalServerError(
                "failed to convert std udp socket to tokio udp socket",
            )
        })?;
        Ok((listen_addr, socket))
    }

    pub(super) fn get_mapped_udp_listen_ip(&self, bind_ip: IpAddr) -> IpAddr {
        self.server_config
            .auto_reply_local_ip_map
            .as_ref()
            .map(|map| map.get(&bind_ip).copied().unwrap_or(bind_ip))
            .unwrap_or(bind_ip)
    }
}
