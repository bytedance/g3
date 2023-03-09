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

mod stats;
pub use stats::{ArcLimitedRecvStats, ArcLimitedSendStats, LimitedRecvStats, LimitedSendStats};

mod recv;
mod send;

pub use recv::{AsyncUdpRecv, LimitedUdpRecv};
pub use send::{AsyncUdpSend, LimitedUdpSend};

mod relay;

pub use relay::{
    UdpRelayClientError, UdpRelayClientRecv, UdpRelayClientSend, UdpRelayRemoteError,
    UdpRelayRemoteRecv, UdpRelayRemoteSend,
};
pub use relay::{UdpRelayClientToRemote, UdpRelayError, UdpRelayRemoteToClient};

mod copy;
pub use copy::{
    UdpCopyClientError, UdpCopyClientRecv, UdpCopyClientSend, UdpCopyRemoteError,
    UdpCopyRemoteRecv, UdpCopyRemoteSend,
};
pub use copy::{UdpCopyClientToRemote, UdpCopyError, UdpCopyRemoteToClient};

mod split;

pub use split::{
    split as split_udp, RecvHalf as UdpRecvHalf, ReuniteError as UdpReuniteError,
    SendHalf as UdpSendHalf,
};

const DEFAULT_UDP_PACKET_SIZE: usize = 4096; // at least for DNS with extension
const DEFAULT_UDP_RELAY_YIELD_SIZE: usize = 1024 * 1024; // 1MB
const MINIMUM_UDP_PACKET_SIZE: usize = 512;
const MAXIMUM_UDP_PACKET_SIZE: usize = 64 * 1024;
const MINIMUM_UDP_RELAY_YIELD_SIZE: usize = 256 * 1024;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct LimitedUdpRelayConfig {
    packet_size: usize,
    yield_size: usize,
}

impl Default for LimitedUdpRelayConfig {
    fn default() -> Self {
        LimitedUdpRelayConfig {
            packet_size: DEFAULT_UDP_PACKET_SIZE,
            yield_size: DEFAULT_UDP_RELAY_YIELD_SIZE,
        }
    }
}

impl LimitedUdpRelayConfig {
    pub fn set_packet_size(&mut self, packet_size: usize) {
        self.packet_size = packet_size.clamp(MINIMUM_UDP_PACKET_SIZE, MAXIMUM_UDP_PACKET_SIZE)
    }

    #[inline]
    pub fn packet_size(&self) -> usize {
        self.packet_size
    }

    pub fn set_yield_size(&mut self, yield_size: usize) {
        self.yield_size = yield_size.max(MINIMUM_UDP_RELAY_YIELD_SIZE);
    }
}
