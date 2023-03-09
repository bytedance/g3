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

use g3_io_ext::{LimitedUdpRecv, LimitedUdpSend};

use super::{ProxySocks5Escaper, ProxySocks5EscaperStats};
use crate::module::tcp_connect::TcpConnectTaskNotes;
use crate::module::udp_connect::{
    ArcUdpConnectTaskRemoteStats, UdpConnectError, UdpConnectResult, UdpConnectTaskNotes,
};
use crate::serve::ServerTaskNotes;

mod stats;
use stats::ProxySocks5UdpConnectRemoteStats;

mod recv;
mod send;

use recv::ProxySocks5UdpConnectRemoteRecv;
use send::ProxySocks5UdpConnectRemoteSend;

impl ProxySocks5Escaper {
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

        let mut tcp_notes = TcpConnectTaskNotes::empty();
        let (tcp_close_receiver, udp_socket, udp_local_addr, udp_peer_addr) = self
            .timed_socks5_udp_associate(udp_notes.buf_conf, &mut tcp_notes, task_notes)
            .await
            .map_err(UdpConnectError::SetupSocketFailed)?;

        udp_notes.local = Some(udp_local_addr);
        udp_notes.next = Some(udp_peer_addr);

        let mut wrapper_stats = ProxySocks5UdpConnectRemoteStats::new(&self.stats, task_stats);
        wrapper_stats.push_user_io_stats(self.fetch_user_upstream_io_stats(task_notes));
        let (ups_r_stats, ups_w_stats) = wrapper_stats.into_pair();

        let (recv, send) = g3_io_ext::split_udp(udp_socket);
        let recv = LimitedUdpRecv::new(
            recv,
            self.config.general.udp_sock_speed_limit.shift_millis,
            self.config.general.udp_sock_speed_limit.max_south_packets,
            self.config.general.udp_sock_speed_limit.max_south_bytes,
            ups_r_stats,
        );
        let send = LimitedUdpSend::new(
            send,
            self.config.general.udp_sock_speed_limit.shift_millis,
            self.config.general.udp_sock_speed_limit.max_north_packets,
            self.config.general.udp_sock_speed_limit.max_north_bytes,
            ups_w_stats,
        );

        let recv = ProxySocks5UdpConnectRemoteRecv::new(recv, tcp_close_receiver);
        let send = ProxySocks5UdpConnectRemoteSend::new(send, upstream.clone());

        Ok((Box::new(recv), Box::new(send), self.escape_logger.clone()))
    }
}
