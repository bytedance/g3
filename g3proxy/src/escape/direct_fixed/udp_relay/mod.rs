/*
 * SPDX-License-Identifier: Apache-2.0
 * Copyright 2023-2025 ByteDance and/or its affiliates.
 */

use std::net::SocketAddr;
use std::sync::Arc;

use g3_io_ext::{LimitedUdpRecv, LimitedUdpSend, UdpRecvHalf, UdpSendHalf};
use g3_socket::util::AddressFamily;

use tokio::net::UdpSocket;

use super::{DirectFixedEscaper, DirectFixedEscaperStats};
use crate::module::udp_relay::{
    ArcUdpRelayTaskRemoteStats, UdpRelayRemoteWrapperStats, UdpRelaySetupError,
    UdpRelaySetupResult, UdpRelayTaskConf,
};
use crate::serve::ServerTaskNotes;

mod recv;
mod send;

pub(crate) use recv::DirectUdpRelayRemoteRecv;
pub(crate) use send::DirectUdpRelayRemoteSend;

impl DirectFixedEscaper {
    pub(super) async fn udp_setup_relay(
        &self,
        task_conf: &UdpRelayTaskConf<'_>,
        task_notes: &ServerTaskNotes,
        task_stats: ArcUdpRelayTaskRemoteStats,
    ) -> UdpRelaySetupResult {
        let mut wrapper_stats = UdpRelayRemoteWrapperStats::new(self.stats.clone(), task_stats);
        wrapper_stats.push_user_io_stats(self.fetch_user_upstream_io_stats(task_notes));
        let wrapper_stats = Arc::new(wrapper_stats);

        let mut recv = DirectUdpRelayRemoteRecv::<LimitedUdpRecv<UdpRecvHalf>>::new();
        let mut send = DirectUdpRelayRemoteSend::<LimitedUdpSend<UdpSendHalf>>::new(
            &self.stats,
            task_notes.user_ctx(),
            &self.egress_net_filter,
            &self.resolver_handle,
            self.config.resolve_strategy,
        );

        if !self.config.no_ipv4 {
            let (bind, r, w) =
                self.get_relay_socket(AddressFamily::Ipv4, task_conf, task_notes, &wrapper_stats)?;
            recv.enable_v4(r, bind);
            send.enable_v4(w, bind);
        }

        if !self.config.no_ipv6 {
            let (bind, r, w) =
                self.get_relay_socket(AddressFamily::Ipv6, task_conf, task_notes, &wrapper_stats)?;
            recv.enable_v6(r, bind);
            send.enable_v6(w, bind);
        }

        Ok((Box::new(recv), Box::new(send), self.escape_logger.clone()))
    }

    fn get_relay_socket(
        &self,
        family: AddressFamily,
        task_conf: &UdpRelayTaskConf<'_>,
        task_notes: &ServerTaskNotes,
        stats: &Arc<UdpRelayRemoteWrapperStats>,
    ) -> Result<
        (
            SocketAddr,
            LimitedUdpRecv<UdpRecvHalf>,
            LimitedUdpSend<UdpSendHalf>,
        ),
        UdpRelaySetupError,
    > {
        let bind = self.get_bind_random(family, task_notes);

        let misc_opts = if let Some(user_ctx) = task_notes.user_ctx() {
            user_ctx
                .user_config()
                .udp_remote_misc_opts(&self.config.udp_misc_opts)
        } else {
            self.config.udp_misc_opts
        };

        let (socket, bind_addr) =
            g3_socket::udp::new_std_bind_relay(&bind, family, task_conf.sock_buf, misc_opts)
                .map_err(UdpRelaySetupError::SetupSocketFailed)?;
        let socket = UdpSocket::from_std(socket).map_err(UdpRelaySetupError::SetupSocketFailed)?;

        let (recv, send) = g3_io_ext::split_udp(socket);
        let recv = LimitedUdpRecv::local_limited(
            recv,
            self.config.general.udp_sock_speed_limit.shift_millis,
            self.config.general.udp_sock_speed_limit.max_south_packets,
            self.config.general.udp_sock_speed_limit.max_south_bytes,
            stats.clone(),
        );
        let send = LimitedUdpSend::local_limited(
            send,
            self.config.general.udp_sock_speed_limit.shift_millis,
            self.config.general.udp_sock_speed_limit.max_north_packets,
            self.config.general.udp_sock_speed_limit.max_north_bytes,
            stats.clone(),
        );

        Ok((bind_addr, recv, send))
    }
}
