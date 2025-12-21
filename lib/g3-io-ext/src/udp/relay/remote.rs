/*
 * SPDX-License-Identifier: Apache-2.0
 * Copyright 2023-2025 ByteDance and/or its affiliates.
 */

use std::io;
use std::net::SocketAddr;
use std::task::{Context, Poll};

#[cfg(feature = "log")]
use slog::Logger;
use thiserror::Error;

#[cfg(feature = "resolver")]
use g3_resolver::ResolveError;
use g3_types::net::UpstreamAddr;

use super::UdpRelayPacket;

#[derive(Error, Debug)]
pub enum UdpRelayRemoteError {
    #[error("no listen socket")]
    NoListenSocket,
    #[error("recv failed: (bind: {0}) {1:?}")]
    RecvFailed(SocketAddr, io::Error),
    #[error("send failed: (bind: {0}, remote: {1}) {2:?}")]
    SendFailed(SocketAddr, SocketAddr, io::Error),
    #[error("batch send failed: (bind: {0}) {1:?}")]
    BatchSendFailed(SocketAddr, io::Error),
    #[error("invalid packet: (bind: {0}) {0}")]
    InvalidPacket(SocketAddr, String),
    #[error("address not supported")]
    AddressNotSupported,
    #[cfg(feature = "resolver")]
    #[error("domain not resolved: {0}")]
    DomainNotResolved(#[from] ResolveError),
    #[error("forbidden target ip address: {0}")]
    ForbiddenTargetIpAddress(SocketAddr),
    #[error("remote session closed")]
    RemoteSessionClosed(SocketAddr, SocketAddr),
    #[error("remote session error: {0:?}")]
    RemoteSessionError(SocketAddr, SocketAddr, io::Error),
    #[error("internal server error: {0}")]
    InternalServerError(&'static str),
}

pub trait UdpRelayRemoteRecv {
    #[cfg(feature = "log")]
    fn error_logger(&self) -> Option<&Logger>;

    /// reserve some space for offloading header
    fn max_hdr_len(&self) -> usize;

    /// return `(off, len. from)`
    fn poll_recv_packet(
        &mut self,
        cx: &mut Context<'_>,
        buf: &mut [u8],
    ) -> Poll<Result<(usize, usize, UpstreamAddr), UdpRelayRemoteError>>;

    #[cfg(any(
        target_os = "linux",
        target_os = "android",
        target_os = "freebsd",
        target_os = "netbsd",
        target_os = "openbsd",
        target_os = "macos",
        target_os = "solaris",
    ))]
    fn poll_recv_packets(
        &mut self,
        cx: &mut Context<'_>,
        packets: &mut [UdpRelayPacket],
    ) -> Poll<Result<usize, UdpRelayRemoteError>>;
}

pub trait UdpRelayRemoteSend {
    #[cfg(feature = "log")]
    fn error_logger(&self) -> Option<&Logger>;

    /// return `nw`, which should be greater than 0
    fn poll_send_packet(
        &mut self,
        cx: &mut Context<'_>,
        buf: &[u8],
        to: &UpstreamAddr,
    ) -> Poll<Result<usize, UdpRelayRemoteError>>;

    fn poll_send_packets(
        &mut self,
        cx: &mut Context<'_>,
        packets: &[UdpRelayPacket],
    ) -> Poll<Result<usize, UdpRelayRemoteError>> {
        let mut count = 0;
        for packet in packets {
            match self.poll_send_packet(cx, packet.payload(), packet.upstream()) {
                Poll::Pending => {
                    return if count > 0 {
                        Poll::Ready(Ok(count))
                    } else {
                        Poll::Pending
                    };
                }
                Poll::Ready(Ok(_)) => count += 1,
                Poll::Ready(Err(e)) => return Poll::Ready(Err(e)),
            }
        }
        Poll::Ready(Ok(count))
    }
}
