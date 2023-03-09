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

#[derive(Error, Debug)]
pub enum UdpCopyRemoteError {
    #[error("recv failed: {0:?}")]
    RecvFailed(io::Error),
    #[error("send failed: {0:?}")]
    SendFailed(io::Error),
    #[error("invalid packet: {0}")]
    InvalidPacket(String),
    #[error("remote session closed")]
    RemoteSessionClosed,
    #[error("remote session error: {0:?}")]
    RemoteSessionError(io::Error),
    #[error("internal server error: {0}")]
    InternalServerError(&'static str),
}

pub trait UdpCopyRemoteRecv {
    /// reserve some space for offloading header
    fn buf_reserve_length(&self) -> usize;

    /// return `(off, len. from)`
    fn poll_recv_packet(
        &mut self,
        cx: &mut Context<'_>,
        buf: &mut [u8],
    ) -> Poll<Result<(usize, usize), UdpCopyRemoteError>>;
}

pub trait UdpCopyRemoteSend {
    /// reserve some space for adding header
    fn buf_reserve_length(&self) -> usize;

    /// return `nw`, which should be greater than 0
    fn poll_send_packet(
        &mut self,
        cx: &mut Context<'_>,
        buf: &mut [u8],
        buf_off: usize,
        buf_len: usize,
    ) -> Poll<Result<usize, UdpCopyRemoteError>>;
}
