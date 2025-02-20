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

use anyhow::anyhow;
use tokio::net::UdpSocket;

use g3_io_ext::{LimitedUdpRecv, LimitedUdpSend, UdpRecvHalf, UdpSendHalf};
use g3_socket::BindAddr;
use g3_socket::util::AddressFamily;

use super::DirectFloatEscaper;
use crate::escape::direct_fixed::DirectFixedEscaperStats;
use crate::escape::direct_fixed::udp_relay::{DirectUdpRelayRemoteRecv, DirectUdpRelayRemoteSend};
use crate::module::udp_relay::{
    ArcUdpRelayTaskRemoteStats, UdpRelayRemoteWrapperStats, UdpRelaySetupError,
    UdpRelaySetupResult, UdpRelayTaskConf,
};
use crate::serve::ServerTaskNotes;

impl DirectFloatEscaper {
    pub(super) async fn udp_setup_relay(
        &self,
        task_conf: &UdpRelayTaskConf<'_>,
        task_notes: &ServerTaskNotes,
        task_stats: ArcUdpRelayTaskRemoteStats,
    ) -> UdpRelaySetupResult {
        let mut wrapper_stats = UdpRelayRemoteWrapperStats::new(&self.stats, task_stats);
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
            if let Ok((bind, r, w)) =
                self.get_relay_socket(AddressFamily::Ipv4, task_conf, task_notes, &wrapper_stats)
            {
                recv.enable_v4(r, bind);
                send.enable_v4(w, bind);
            }
        }

        if !self.config.no_ipv6 {
            if let Ok((bind, r, w)) =
                self.get_relay_socket(AddressFamily::Ipv6, task_conf, task_notes, &wrapper_stats)
            {
                recv.enable_v6(r, bind);
                send.enable_v6(w, bind);
            }
        }

        if !send.usable() {
            return Err(UdpRelaySetupError::EscaperNotUsable(anyhow!(
                "no ipv4 / ipv6 bind address found"
            )));
        }

        Ok((Box::new(recv), Box::new(send), self.escape_logger.clone()))
    }

    fn get_relay_socket(
        &self,
        family: AddressFamily,
        task_conf: &UdpRelayTaskConf<'_>,
        task_notes: &ServerTaskNotes,
        stats: &Arc<UdpRelayRemoteWrapperStats<DirectFixedEscaperStats>>,
    ) -> Result<
        (
            SocketAddr,
            LimitedUdpRecv<UdpRecvHalf>,
            LimitedUdpSend<UdpSendHalf>,
        ),
        UdpRelaySetupError,
    > {
        let bind = self
            .select_bind(family, task_notes)
            .map_err(UdpRelaySetupError::EscaperNotUsable)?;

        let misc_opts = if let Some(user_ctx) = task_notes.user_ctx() {
            user_ctx
                .user_config()
                .udp_remote_misc_opts(&self.config.udp_misc_opts)
        } else {
            self.config.udp_misc_opts
        };

        let socket = g3_socket::udp::new_std_bind_relay(
            &BindAddr::Ip(bind.ip),
            family,
            task_conf.sock_buf,
            misc_opts,
        )
        .map_err(UdpRelaySetupError::SetupSocketFailed)?;
        let bind_addr = socket
            .local_addr()
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
