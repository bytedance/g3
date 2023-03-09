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

use thiserror::Error;

#[derive(Error, Debug)]
pub enum SocksNegotiationError {
    #[error("invalid version code")]
    InvalidVersion,
    #[error("no auth method set in request")]
    NoAuthMethod,
    #[error("invalid auth method")]
    InvalidAuthMethod,
    #[error("invalid command code")]
    InvalidCommand,
    #[error("invalid user id string")]
    InvalidUserIdString,
    #[error("invalid domain string")]
    InvalidDomainString,
    #[error("invalid addr type")]
    InvalidAddrType,
    #[error("invalid user auth message")]
    InvalidUserAuthMsg,
}

#[derive(Error, Debug)]
pub enum SocksUdpPacketError {
    #[error("too small packet")]
    TooSmallPacket,
    #[error("reserved not zeroed")]
    ReservedNotZeroed,
    #[error("fragment not supported")]
    FragmentNotSupported,
    #[error("invalid domain string")]
    InvalidDomainString,
    #[error("invalid addr type")]
    InvalidAddrType,
}

#[derive(Error, Debug)]
pub enum SocksRequestParseError {
    #[error("read failed: {0:?}")]
    ReadFailed(io::Error),
    #[error("invalid socks protocol: {0}")]
    InvalidProtocol(#[from] SocksNegotiationError),
    #[error("invalid udp peer address")]
    InvalidUdpPeerAddress,
    #[error("client closed")]
    ClientClosed,
}

impl From<io::Error> for SocksRequestParseError {
    fn from(e: io::Error) -> Self {
        if matches!(e.kind(), io::ErrorKind::UnexpectedEof) {
            SocksRequestParseError::ClientClosed
        } else {
            SocksRequestParseError::ReadFailed(e)
        }
    }
}

#[derive(Error, Debug)]
pub enum SocksReplyParseError {
    #[error("read failed: {0:?}")]
    ReadFailed(#[from] io::Error),
    #[error("invalid socks protocol: {0}")]
    InvalidProtocol(#[from] SocksNegotiationError),
}

#[derive(Error, Debug)]
pub enum SocksConnectError {
    #[error("read failed: {0:?}")]
    ReadFailed(io::Error),
    #[error("write failed: {0:?}")]
    WriteFailed(io::Error),
    #[error("no auth method available")]
    NoAuthMethodAvailable,
    #[error("unsupported auth version")]
    UnsupportedAuthVersion,
    #[error("auth failed")]
    AuthFailed,
    #[error("invalid socks protocol: {0}")]
    InvalidProtocol(#[from] SocksNegotiationError),
    #[error("peer timeout")]
    PeerTimeout,
    #[error("request failed: {0}")]
    RequestFailed(String),
}

impl From<SocksReplyParseError> for SocksConnectError {
    fn from(e: SocksReplyParseError) -> Self {
        match e {
            SocksReplyParseError::ReadFailed(e) => SocksConnectError::ReadFailed(e),
            SocksReplyParseError::InvalidProtocol(e) => SocksConnectError::InvalidProtocol(e),
        }
    }
}
