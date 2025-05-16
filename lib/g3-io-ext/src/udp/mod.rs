/*
 * SPDX-License-Identifier: Apache-2.0
 * Copyright 2023-2025 ByteDance and/or its affiliates.
 */

mod stats;
pub use stats::{ArcLimitedRecvStats, ArcLimitedSendStats, LimitedRecvStats, LimitedSendStats};

mod ext;
pub use ext::*;

mod recv;
mod send;

pub use recv::{AsyncUdpRecv, LimitedUdpRecv};
pub use send::{AsyncUdpSend, LimitedUdpSend};

mod relay;
pub use relay::{
    UdpRelayClientError, UdpRelayClientRecv, UdpRelayClientSend, UdpRelayPacket,
    UdpRelayPacketMeta, UdpRelayRemoteError, UdpRelayRemoteRecv, UdpRelayRemoteSend,
};
pub use relay::{UdpRelayClientToRemote, UdpRelayError, UdpRelayRemoteToClient};

mod copy;
pub use copy::{
    UdpCopyClientError, UdpCopyClientRecv, UdpCopyClientSend, UdpCopyPacket, UdpCopyPacketMeta,
    UdpCopyRemoteError, UdpCopyRemoteRecv, UdpCopyRemoteSend,
};
pub use copy::{UdpCopyClientToRemote, UdpCopyError, UdpCopyRemoteToClient};

mod split;
pub use split::{
    RecvHalf as UdpRecvHalf, ReuniteError as UdpReuniteError, SendHalf as UdpSendHalf,
    split as split_udp,
};

const DEFAULT_UDP_PACKET_SIZE: usize = 4096; // at least for DNS with extension
const DEFAULT_UDP_RELAY_YIELD_SIZE: usize = 1024 * 1024; // 1MB
const DEFAULT_UDP_BATCH_SIZE: usize = 8;
const MINIMUM_UDP_PACKET_SIZE: usize = 512;
const MAXIMUM_UDP_PACKET_SIZE: usize = 64 * 1024;
const MINIMUM_UDP_RELAY_YIELD_SIZE: usize = 256 * 1024;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct LimitedUdpRelayConfig {
    packet_size: usize,
    yield_size: usize,
    batch_size: usize,
}

impl Default for LimitedUdpRelayConfig {
    fn default() -> Self {
        LimitedUdpRelayConfig {
            packet_size: DEFAULT_UDP_PACKET_SIZE,
            yield_size: DEFAULT_UDP_RELAY_YIELD_SIZE,
            batch_size: DEFAULT_UDP_BATCH_SIZE,
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

    pub fn set_batch_size(&mut self, batch_size: usize) {
        self.batch_size = batch_size;
    }
}
