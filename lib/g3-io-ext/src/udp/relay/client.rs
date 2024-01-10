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

use std::io;
use std::task::{Context, Poll};

use thiserror::Error;

use g3_types::net::UpstreamAddr;

#[cfg(any(
    target_os = "linux",
    target_os = "android",
    target_os = "freebsd",
    target_os = "netbsd",
    target_os = "openbsd",
))]
use super::UdpRelayPacket;

#[derive(Error, Debug)]
pub enum UdpRelayClientError {
    #[error("recv failed: {0:?}")]
    RecvFailed(io::Error),
    #[error("send failed: {0:?}")]
    SendFailed(io::Error),
    #[error("address not supported")]
    AddressNotSupported,
    #[error("invalid packet: {0}")]
    InvalidPacket(String),
    #[error("mismatched client address")]
    MismatchedClientAddress,
    #[error("forbidden client address")]
    ForbiddenClientAddress,
    #[error("forbidden target address")]
    ForbiddenTargetAddress,
}

pub trait UdpRelayClientRecv {
    /// reserve some space for offloading header
    fn max_hdr_len(&self) -> usize;

    /// return `(off, len, to)`
    fn poll_recv_packet(
        &mut self,
        cx: &mut Context<'_>,
        buf: &mut [u8],
    ) -> Poll<Result<(usize, usize, UpstreamAddr), UdpRelayClientError>>;

    #[cfg(any(
        target_os = "linux",
        target_os = "android",
        target_os = "freebsd",
        target_os = "netbsd",
        target_os = "openbsd",
    ))]
    fn poll_recv_packets(
        &mut self,
        cx: &mut Context<'_>,
        packets: &mut [UdpRelayPacket],
    ) -> Poll<Result<usize, UdpRelayClientError>>;
}

pub trait UdpRelayClientSend {
    /// return `nw`, which should be greater than 0
    fn poll_send_packet(
        &mut self,
        cx: &mut Context<'_>,
        buf: &[u8],
        from: &UpstreamAddr,
    ) -> Poll<Result<usize, UdpRelayClientError>>;

    #[cfg(any(
        target_os = "linux",
        target_os = "android",
        target_os = "freebsd",
        target_os = "netbsd",
        target_os = "openbsd",
    ))]
    fn poll_send_packets(
        &mut self,
        cx: &mut Context<'_>,
        packets: &[UdpRelayPacket],
    ) -> Poll<Result<usize, UdpRelayClientError>>;
}
