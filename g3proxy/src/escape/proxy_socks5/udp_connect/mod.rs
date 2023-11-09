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

use g3_io_ext::{LimitedUdpRecv, LimitedUdpSend};

use super::ProxySocks5Escaper;
use crate::module::tcp_connect::TcpConnectTaskNotes;
use crate::module::udp_connect::{
    ArcUdpConnectTaskRemoteStats, UdpConnectError, UdpConnectRemoteWrapperStats, UdpConnectResult,
    UdpConnectTaskNotes,
};
use crate::serve::ServerTaskNotes;

mod recv;
mod send;

pub(crate) use recv::ProxySocks5UdpConnectRemoteRecv;
pub(crate) use send::ProxySocks5UdpConnectRemoteSend;

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

        let mut wrapper_stats = UdpConnectRemoteWrapperStats::new(&self.stats, task_stats);
        wrapper_stats.push_user_io_stats(self.fetch_user_upstream_io_stats(task_notes));
        let wrapper_stats = Arc::new(wrapper_stats);

        let (recv, send) = g3_io_ext::split_udp(udp_socket);
        let recv = LimitedUdpRecv::new(
            recv,
            self.config.general.udp_sock_speed_limit.shift_millis,
            self.config.general.udp_sock_speed_limit.max_south_packets,
            self.config.general.udp_sock_speed_limit.max_south_bytes,
            wrapper_stats.clone() as _,
        );
        let send = LimitedUdpSend::new(
            send,
            self.config.general.udp_sock_speed_limit.shift_millis,
            self.config.general.udp_sock_speed_limit.max_north_packets,
            self.config.general.udp_sock_speed_limit.max_north_bytes,
            wrapper_stats as _,
        );

        let recv = ProxySocks5UdpConnectRemoteRecv::new(recv, tcp_close_receiver);
        let send = ProxySocks5UdpConnectRemoteSend::new(send, upstream);

        Ok((Box::new(recv), Box::new(send), self.escape_logger.clone()))
    }
}
