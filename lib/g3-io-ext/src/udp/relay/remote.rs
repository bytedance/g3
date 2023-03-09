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
use std::net::SocketAddr;
use std::task::{Context, Poll};

use thiserror::Error;

use g3_resolver::ResolveError;
use g3_types::net::UpstreamAddr;

#[derive(Error, Debug)]
pub enum UdpRelayRemoteError {
    #[error("no listen socket")]
    NoListenSocket,
    #[error("recv failed: (bind: {0}) {1:?}")]
    RecvFailed(SocketAddr, io::Error),
    #[error("send failed: (bind: {0}, remote: {1}) {2:?}")]
    SendFailed(SocketAddr, SocketAddr, io::Error),
    #[error("invalid packet: (bind: {0}) {0}")]
    InvalidPacket(SocketAddr, String),
    #[error("address not supported")]
    AddressNotSupported,
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
    /// reserve some space for offloading header
    fn buf_reserve_length(&self) -> usize;

    /// return `(off, len. from)`
    fn poll_recv_packet(
        &mut self,
        cx: &mut Context<'_>,
        buf: &mut [u8],
    ) -> Poll<Result<(usize, usize, UpstreamAddr), UdpRelayRemoteError>>;
}

pub trait UdpRelayRemoteSend {
    /// reserve some space for adding header
    fn buf_reserve_length(&self) -> usize;

    /// return `nw`, which should be greater than 0
    fn poll_send_packet(
        &mut self,
        cx: &mut Context<'_>,
        buf: &mut [u8],
        buf_off: usize,
        buf_len: usize,
        to: &UpstreamAddr,
    ) -> Poll<Result<usize, UdpRelayRemoteError>>;
}
