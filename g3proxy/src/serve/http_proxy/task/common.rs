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

use std::net::SocketAddr;
use std::os::unix::prelude::RawFd;
use std::sync::Arc;

use slog::Logger;

use g3_icap_client::reqmod::h1::HttpAdapterErrorResponse;
use g3_types::acl::AclAction;
use g3_types::acl_set::AclDstHostRuleSet;
use g3_types::net::{OpensslTlsClientConfig, UpstreamAddr};

use super::{HttpProxyServerConfig, HttpProxyServerStats};
use crate::audit::AuditHandle;
use crate::escape::ArcEscaper;
use crate::module::http_forward::HttpProxyClientResponse;
use crate::module::http_header;
use crate::module::tcp_connect::TcpConnectTaskNotes;
use crate::serve::{ServerIdleChecker, ServerQuitPolicy, ServerTaskNotes};

#[derive(Clone)]
pub(crate) struct CommonTaskContext {
    pub(crate) server_config: Arc<HttpProxyServerConfig>,
    pub(crate) server_stats: Arc<HttpProxyServerStats>,
    pub(crate) server_quit_policy: Arc<ServerQuitPolicy>,
    pub(crate) escaper: ArcEscaper,
    pub(crate) audit_handle: Option<Arc<AuditHandle>>,
    pub(crate) tcp_server_addr: SocketAddr,
    pub(crate) tcp_client_addr: SocketAddr,
    pub(crate) tls_client_config: Arc<OpensslTlsClientConfig>,
    pub(crate) task_logger: Logger,
    pub(crate) worker_id: Option<usize>,

    pub(crate) dst_host_filter: Option<Arc<AclDstHostRuleSet>>,
    pub(crate) tcp_client_socket: RawFd,
}

impl CommonTaskContext {
    pub(crate) fn idle_checker(&self, task_notes: &ServerTaskNotes) -> ServerIdleChecker {
        ServerIdleChecker {
            idle_duration: self.server_config.task_idle_check_duration,
            user: task_notes.user_ctx().map(|ctx| ctx.user().clone()),
            task_max_idle_count: self.server_config.task_idle_max_count,
            server_quit_policy: self.server_quit_policy.clone(),
        }
    }

    pub(crate) fn check_upstream(&self, upstream: &UpstreamAddr) -> AclAction {
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

    pub(crate) fn set_custom_header_for_local_reply(
        &self,
        tcp_notes: &TcpConnectTaskNotes,
        rsp: &mut HttpProxyClientResponse,
    ) {
        if let Some(server_id) = &self.server_config.server_id {
            let line = http_header::remote_connection_info(
                server_id,
                tcp_notes.bind,
                tcp_notes.local,
                tcp_notes.next,
                &tcp_notes.expire,
            );
            rsp.add_extra_header(line);

            if let Some(egress_info) = &tcp_notes.egress {
                let line = http_header::dynamic_egress_info(server_id, egress_info);
                rsp.add_extra_header(line);
            }
        }

        if self.server_config.echo_chained_info {
            if let Some(addr) = tcp_notes.chained.target_addr {
                rsp.set_upstream_addr(addr);
            }

            if let Some(addr) = tcp_notes.chained.outgoing_addr {
                rsp.set_outgoing_ip(addr.ip());
            }
        }
    }

    pub(crate) fn set_custom_header_for_adaptation_error_reply(
        &self,
        tcp_notes: &TcpConnectTaskNotes,
        rsp: &mut HttpAdapterErrorResponse,
    ) {
        if let Some(server_id) = &self.server_config.server_id {
            http_header::set_remote_connection_info(
                &mut rsp.headers,
                server_id,
                tcp_notes.bind,
                tcp_notes.local,
                tcp_notes.next,
                &tcp_notes.expire,
            );

            if let Some(egress_info) = &tcp_notes.egress {
                http_header::set_dynamic_egress_info(&mut rsp.headers, server_id, egress_info);
            }
        }

        if self.server_config.echo_chained_info {
            if let Some(addr) = tcp_notes.chained.target_addr {
                http_header::set_upstream_addr(&mut rsp.headers, addr);
            }

            if let Some(addr) = tcp_notes.chained.outgoing_addr {
                http_header::set_outgoing_ip(&mut rsp.headers, addr);
            }
        }
    }
}
