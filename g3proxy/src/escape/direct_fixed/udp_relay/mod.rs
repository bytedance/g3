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
use std::sync::Arc;

use g3_io_ext::{LimitedUdpRecv, LimitedUdpSend, UdpRecvHalf, UdpSendHalf};
use g3_socket::util::AddressFamily;

use tokio::net::UdpSocket;

use super::{DirectFixedEscaper, DirectFixedEscaperStats};
use crate::module::udp_relay::{
    ArcUdpRelayTaskRemoteStats, UdpRelaySetupError, UdpRelaySetupResult, UdpRelayTaskNotes,
};
use crate::serve::ServerTaskNotes;

mod recv;
mod send;
mod stats;

use recv::DirectUdpRelayRemoteRecv;
use send::DirectUdpRelayRemoteSend;
use stats::DirectUdpRelayRemoteStats;

impl DirectFixedEscaper {
    pub(super) async fn udp_setup_relay<'a>(
        &'a self,
        udp_notes: &'a mut UdpRelayTaskNotes,
        task_notes: &'a ServerTaskNotes,
        task_stats: ArcUdpRelayTaskRemoteStats,
    ) -> UdpRelaySetupResult {
        let mut wrapper_stats = DirectUdpRelayRemoteStats::new(&self.stats, task_stats);
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
                self.get_relay_socket(AddressFamily::Ipv4, udp_notes, task_notes, &wrapper_stats)?;
            recv.enable_v4(r, bind);
            send.enable_v4(w, bind);
        }

        if !self.config.no_ipv6 {
            let (bind, r, w) =
                self.get_relay_socket(AddressFamily::Ipv6, udp_notes, task_notes, &wrapper_stats)?;
            recv.enable_v6(r, bind);
            send.enable_v6(w, bind);
        }

        Ok((Box::new(recv), Box::new(send), self.escape_logger.clone()))
    }

    fn get_relay_socket(
        &self,
        family: AddressFamily,
        udp_notes: &UdpRelayTaskNotes,
        task_notes: &ServerTaskNotes,
        stats: &Arc<DirectUdpRelayRemoteStats>,
    ) -> Result<
        (
            SocketAddr,
            LimitedUdpRecv<UdpRecvHalf>,
            LimitedUdpSend<UdpSendHalf>,
        ),
        UdpRelaySetupError,
    > {
        let bind_ip = self.get_bind_random(family, &task_notes.egress_path_selection);

        let misc_opts = if let Some(user_ctx) = task_notes.user_ctx() {
            user_ctx
                .user()
                .config
                .udp_remote_misc_opts(&self.config.udp_misc_opts)
        } else {
            self.config.udp_misc_opts
        };

        let socket =
            g3_socket::udp::new_std_bind_relay(bind_ip, family, udp_notes.buf_conf, &misc_opts)
                .map_err(UdpRelaySetupError::SetupSocketFailed)?;
        let bind_addr = socket
            .local_addr()
            .map_err(UdpRelaySetupError::SetupSocketFailed)?;
        let socket = UdpSocket::from_std(socket).map_err(UdpRelaySetupError::SetupSocketFailed)?;

        let (recv, send) = g3_io_ext::split_udp(socket);
        let recv = LimitedUdpRecv::new(
            recv,
            self.config.general.udp_sock_speed_limit.shift_millis,
            self.config.general.udp_sock_speed_limit.max_south_packets,
            self.config.general.udp_sock_speed_limit.max_south_bytes,
            stats.for_recv(),
        );
        let send = LimitedUdpSend::new(
            send,
            self.config.general.udp_sock_speed_limit.shift_millis,
            self.config.general.udp_sock_speed_limit.max_north_packets,
            self.config.general.udp_sock_speed_limit.max_north_bytes,
            stats.for_send(),
        );

        Ok((bind_addr, recv, send))
    }
}
