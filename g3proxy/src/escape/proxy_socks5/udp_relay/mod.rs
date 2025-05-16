/*
 * SPDX-License-Identifier: Apache-2.0
 * Copyright 2023-2025 ByteDance and/or its affiliates.
 */

use std::sync::Arc;

use g3_io_ext::{LimitedUdpRecv, LimitedUdpSend};

use super::ProxySocks5Escaper;
use crate::module::tcp_connect::TcpConnectTaskNotes;
use crate::module::udp_relay::{
    ArcUdpRelayTaskRemoteStats, UdpRelayRemoteWrapperStats, UdpRelaySetupError,
    UdpRelaySetupResult, UdpRelayTaskConf,
};
use crate::serve::ServerTaskNotes;

mod recv;
mod send;

pub(crate) use recv::ProxySocks5UdpRelayRemoteRecv;
pub(crate) use send::ProxySocks5UdpRelayRemoteSend;

impl ProxySocks5Escaper {
    pub(super) async fn udp_setup_relay(
        &self,
        task_conf: &UdpRelayTaskConf<'_>,
        task_notes: &ServerTaskNotes,
        task_stats: ArcUdpRelayTaskRemoteStats,
    ) -> UdpRelaySetupResult {
        let mut tcp_notes = TcpConnectTaskNotes::default();
        let (ctl_stream, udp_socket, udp_local_addr, udp_peer_addr) = self
            .timed_socks5_udp_associate(task_conf.sock_buf, &mut tcp_notes, task_notes)
            .await
            .map_err(UdpRelaySetupError::SetupSocketFailed)?;

        let mut wrapper_stats = UdpRelayRemoteWrapperStats::new(self.stats.clone(), task_stats);
        wrapper_stats.push_user_io_stats(self.fetch_user_upstream_io_stats(task_notes));
        let wrapper_stats = Arc::new(wrapper_stats);

        let (recv, send) = g3_io_ext::split_udp(udp_socket);
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

        let recv = ProxySocks5UdpRelayRemoteRecv::new(
            recv,
            udp_local_addr,
            udp_peer_addr,
            ctl_stream,
            self.config.end_on_control_closed,
        );
        let send = ProxySocks5UdpRelayRemoteSend::new(send, udp_local_addr, udp_peer_addr);

        Ok((Box::new(recv), Box::new(send), self.escape_logger.clone()))
    }
}
