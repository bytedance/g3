/*
 * SPDX-License-Identifier: Apache-2.0
 * Copyright 2023-2025 ByteDance and/or its affiliates.
 */

use std::sync::Arc;

use tokio::net::UdpSocket;

use g3_io_ext::{LimitedUdpRecv, LimitedUdpSend};
use g3_socket::util::AddressFamily;
use g3_types::acl::AclAction;

use super::DirectFixedEscaper;
use crate::module::udp_connect::{
    ArcUdpConnectTaskRemoteStats, UdpConnectError, UdpConnectRemoteWrapperStats, UdpConnectResult,
    UdpConnectTaskConf, UdpConnectTaskNotes,
};
use crate::serve::ServerTaskNotes;

mod recv;
mod send;

pub(crate) use recv::DirectUdpConnectRemoteRecv;
pub(crate) use send::DirectUdpConnectRemoteSend;

impl DirectFixedEscaper {
    fn handle_udp_target_ip_acl_action(
        &self,
        action: AclAction,
        task_notes: &ServerTaskNotes,
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

    pub(super) async fn udp_connect_to(
        &self,
        task_conf: &UdpConnectTaskConf<'_>,
        udp_notes: &mut UdpConnectTaskNotes,
        task_notes: &ServerTaskNotes,
        task_stats: ArcUdpConnectTaskRemoteStats,
    ) -> UdpConnectResult {
        let peer_addr = self
            .select_upstream_addr(
                task_conf.upstream,
                self.get_resolve_strategy(task_notes),
                task_notes,
            )
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
            task_conf.sock_buf,
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

        let mut wrapper_stats = UdpConnectRemoteWrapperStats::new(self.stats.clone(), task_stats);
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

        let send = if let Some(decision) = task_notes.sticky()
            && decision.enabled()
            && !decision.rotate
        {
            let key = crate::sticky::build_sticky_key(decision, task_conf.upstream);
            let ttl = decision.effective_ttl();
            Box::new(DirectUdpConnectRemoteSend::new_with_sticky(send, key, ttl))
                as Box<dyn g3_io_ext::UdpCopyRemoteSend + Unpin + Send + Sync>
        } else {
            Box::new(DirectUdpConnectRemoteSend::new(send))
                as Box<dyn g3_io_ext::UdpCopyRemoteSend + Unpin + Send + Sync>
        };

        Ok((
            Box::new(DirectUdpConnectRemoteRecv::new(recv)),
            send,
            self.escape_logger.clone(),
        ))
    }
}
