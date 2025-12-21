/*
 * SPDX-License-Identifier: Apache-2.0
 * Copyright 2023-2025 ByteDance and/or its affiliates.
 */

use std::sync::Arc;

use g3_io_ext::{LimitedUdpRecv, LimitedUdpSend};

use super::{ProxyFloatEscaper, ProxyFloatSocks5sPeer};
use crate::escape::proxy_socks5::udp_relay::{
    ProxySocks5UdpRelayRemoteRecv, ProxySocks5UdpRelayRemoteSend,
};
use crate::module::tcp_connect::TcpConnectTaskNotes;
use crate::module::udp_relay::{
    ArcUdpRelayTaskRemoteStats, UdpRelayRemoteWrapperStats, UdpRelaySetupError,
    UdpRelaySetupResult, UdpRelayTaskConf,
};
use crate::serve::ServerTaskNotes;

impl ProxyFloatSocks5sPeer {
    pub(super) async fn udp_setup_relay(
        &self,
        escaper: &ProxyFloatEscaper,
        task_conf: &UdpRelayTaskConf<'_>,
        task_notes: &ServerTaskNotes,
        task_stats: ArcUdpRelayTaskRemoteStats,
    ) -> UdpRelaySetupResult {
        let mut tcp_notes = TcpConnectTaskNotes::default();
        let (ctl_stream, udp_socket, udp_local_addr, udp_peer_addr) = self
            .timed_socks5_udp_associate(escaper, task_conf.sock_buf, &mut tcp_notes, task_notes)
            .await
            .map_err(UdpRelaySetupError::SetupSocketFailed)?;

        let mut wrapper_stats = UdpRelayRemoteWrapperStats::new(escaper.stats.clone(), task_stats);
        wrapper_stats.push_user_io_stats(escaper.fetch_user_upstream_io_stats(task_notes));
        let wrapper_stats = Arc::new(wrapper_stats);

        let (recv, send) = g3_io_ext::split_udp(udp_socket);
        let recv = LimitedUdpRecv::local_limited(
            recv,
            self.udp_sock_speed_limit.shift_millis,
            self.udp_sock_speed_limit.max_south_packets,
            self.udp_sock_speed_limit.max_south_bytes,
            wrapper_stats.clone(),
        );
        let send = LimitedUdpSend::local_limited(
            send,
            self.udp_sock_speed_limit.shift_millis,
            self.udp_sock_speed_limit.max_north_packets,
            self.udp_sock_speed_limit.max_north_bytes,
            wrapper_stats,
        );

        let recv = ProxySocks5UdpRelayRemoteRecv::new(
            recv,
            udp_local_addr,
            udp_peer_addr,
            ctl_stream,
            self.end_on_control_closed,
            escaper.escape_logger.clone(),
        );
        let send = ProxySocks5UdpRelayRemoteSend::new(
            send,
            udp_local_addr,
            udp_peer_addr,
            escaper.escape_logger.clone(),
        );

        Ok((Box::new(recv), Box::new(send)))
    }
}
