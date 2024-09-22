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

use tokio::net::UdpSocket;

use g3_io_ext::{LimitedUdpRecv, LimitedUdpSend};
use g3_socket::util::AddressFamily;
use g3_types::acl::AclAction;

use super::DirectFixedEscaper;
use crate::module::udp_connect::{
    ArcUdpConnectTaskRemoteStats, UdpConnectError, UdpConnectRemoteWrapperStats, UdpConnectResult,
    UdpConnectTaskNotes,
};
use crate::serve::ServerTaskNotes;

mod recv;
mod send;

pub(crate) use recv::DirectUdpConnectRemoteRecv;
pub(crate) use send::DirectUdpConnectRemoteSend;

impl DirectFixedEscaper {
    fn handle_udp_target_ip_acl_action<'a>(
        &'a self,
        action: AclAction,
        task_notes: &'a ServerTaskNotes,
    ) -> Result<(), UdpConnectError> {
        let forbid = match action {
            AclAction::Permit => false,
            AclAction::PermitAndLog => {
                // TODO log
                false
            }
            AclAction::Forbid => true,
            AclAction::ForbidAndLog => {
                // TODO log
                true
            }
        };
        if forbid {
            self.stats.forbidden.add_ip_blocked();
            if let Some(user_ctx) = task_notes.user_ctx() {
                user_ctx.add_ip_blocked();
            }
            Err(UdpConnectError::ForbiddenRemoteAddress)
        } else {
            Ok(())
        }
    }

    pub(super) async fn udp_connect_to<'a>(
        &'a self,
        udp_notes: &'a mut UdpConnectTaskNotes,
        task_notes: &'a ServerTaskNotes,
        task_stats: ArcUdpConnectTaskRemoteStats,
    ) -> UdpConnectResult {
        let upstream = udp_notes
            .upstream
            .as_ref()
            .ok_or(UdpConnectError::NoUpstreamSupplied)?;
        let peer_addr = self
            .select_upstream_addr(upstream, self.get_resolve_strategy(task_notes), task_notes)
            .await?;
        udp_notes.next = Some(peer_addr);

        let (_, action) = self.egress_net_filter.check(peer_addr.ip());
        self.handle_udp_target_ip_acl_action(action, task_notes)?;

        let family = AddressFamily::from(&peer_addr);
        let bind = self.get_bind_random(family, task_notes.egress_path());
        udp_notes.bind = bind;

        let misc_opts = if let Some(user_ctx) = task_notes.user_ctx() {
            user_ctx
                .user_config()
                .udp_remote_misc_opts(&self.config.udp_misc_opts)
        } else {
            self.config.udp_misc_opts
        };

        let socket = g3_socket::udp::new_std_socket_to(
            peer_addr,
            &udp_notes.bind,
            udp_notes.buf_conf,
            misc_opts,
        )
        .map_err(UdpConnectError::SetupSocketFailed)?;
        socket
            .connect(peer_addr)
            .map_err(UdpConnectError::SetupSocketFailed)?;
        let socket = UdpSocket::from_std(socket).map_err(UdpConnectError::SetupSocketFailed)?;
        let bind_addr = socket
            .local_addr()
            .map_err(UdpConnectError::SetupSocketFailed)?;
        udp_notes.local = Some(bind_addr);

        let mut wrapper_stats = UdpConnectRemoteWrapperStats::new(&self.stats, task_stats);
        wrapper_stats.push_user_io_stats(self.fetch_user_upstream_io_stats(task_notes));
        let wrapper_stats = Arc::new(wrapper_stats);

        let (recv, send) = g3_io_ext::split_udp(socket);
        let recv = LimitedUdpRecv::local_limited(
            recv,
            self.config.general.udp_sock_speed_limit.shift_millis,
            self.config.general.udp_sock_speed_limit.max_south_packets,
            self.config.general.udp_sock_speed_limit.max_south_bytes,
            wrapper_stats.clone(),
        );
        let send = LimitedUdpSend::local_limited(
            send,
            self.config.general.udp_sock_speed_limit.shift_millis,
            self.config.general.udp_sock_speed_limit.max_north_packets,
            self.config.general.udp_sock_speed_limit.max_north_bytes,
            wrapper_stats,
        );

        Ok((
            Box::new(DirectUdpConnectRemoteRecv::new(recv)),
            Box::new(DirectUdpConnectRemoteSend::new(send)),
            self.escape_logger.clone(),
        ))
    }
}
