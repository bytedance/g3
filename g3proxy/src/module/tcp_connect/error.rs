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

use g3_http::connect::HttpConnectError;
use g3_resolver::ResolveError;
use g3_socks::v5::Socks5Reply;
use g3_socks::SocksConnectError;
use g3_types::net::{ConnectError, ProxyProtocolEncodeError};

use crate::serve::{ServerTaskError, ServerTaskForbiddenError};

#[derive(Error, Debug)]
pub(crate) enum TcpConnectError {
    #[error("method is not available")]
    MethodUnavailable,
    #[error("escaper not usable")]
    EscaperNotUsable,
    #[error("resolve failed: {0}")]
    ResolveFailed(#[from] ResolveError),
    #[error("setup socket failed: {0:?}")]
    SetupSocketFailed(io::Error),
    #[error("connect failed: {0}")]
    ConnectFailed(#[from] ConnectError),
    #[error("timeout by rule")]
    TimeoutByRule,
    #[error("no address connected")]
    NoAddressConnected,
    #[error("forbidden address family")]
    ForbiddenAddressFamily,
    #[error("forbidden remote address")]
    ForbiddenRemoteAddress,
    #[error("proxy protocol encode error: {0}")]
    ProxyProtocolEncodeError(ProxyProtocolEncodeError),
    #[error("proxy protocol write failed: {0:?}")]
    ProxyProtocolWriteFailed(io::Error),
    #[error("negotiation read failed: {0:?}")]
    NegotiationReadFailed(io::Error),
    #[error("negotiation write failed: {0:?}")]
    NegotiationWriteFailed(io::Error),
    #[error("negotiation rejected: {0}")]
    NegotiationRejected(String),
    #[error("negotiation timeout")]
    NegotiationPeerTimeout,
    #[error("negotiation protocol error")]
    NegotiationProtocolErr,
    #[error("internal server error: {0}")]
    InternalServerError(&'static str),
    #[error("internal tls client error: {0:?}")]
    InternalTlsClientError(anyhow::Error),
    #[error("peer tls handshake timeout")]
    PeerTlsHandshakeTimeout,
    #[error("peer tls handshake failed: {0:?}")]
    PeerTlsHandshakeFailed(anyhow::Error),
    #[error("upstream tls handshake timeout")]
    UpstreamTlsHandshakeTimeout,
    #[error("upstream tls handshake failed: {0:?}")]
    UpstreamTlsHandshakeFailed(anyhow::Error),
}

impl TcpConnectError {
    pub(crate) fn brief(&self) -> &'static str {
        match self {
            TcpConnectError::MethodUnavailable => "MethodUnavailable",
            TcpConnectError::EscaperNotUsable => "EscaperNotUsable",
            TcpConnectError::ResolveFailed(_) => "ResolveFailed",
            TcpConnectError::SetupSocketFailed(_) => "SetupSocketFailed",
            TcpConnectError::ConnectFailed(_) => "ConnectFailed",
            TcpConnectError::TimeoutByRule => "TimeoutByRule",
            TcpConnectError::NoAddressConnected => "NoAddressConnected",
            TcpConnectError::ForbiddenAddressFamily => "ForbiddenAddressFamily",
            TcpConnectError::ForbiddenRemoteAddress => "ForbiddenRemoteAddress",
            TcpConnectError::ProxyProtocolEncodeError(_) => "ProxyProtocolEncodeError",
            TcpConnectError::ProxyProtocolWriteFailed(_) => "ProxyProtocolWriteFailed",
            TcpConnectError::NegotiationReadFailed(_) => "NegotiationReadFailed",
            TcpConnectError::NegotiationWriteFailed(_) => "NegotiationWriteFailed",
            TcpConnectError::NegotiationRejected(_) => "NegotiationRejected",
            TcpConnectError::NegotiationPeerTimeout => "NegotiationPeerTimeout",
            TcpConnectError::NegotiationProtocolErr => "NegotiationProtocolErr",
            TcpConnectError::InternalServerError(_) => "InternalServerError",
            TcpConnectError::InternalTlsClientError(_) => "InternalTlsClientError",
            TcpConnectError::PeerTlsHandshakeTimeout => "PeerTlsHandshakeTimeout",
            TcpConnectError::PeerTlsHandshakeFailed(_) => "PeerTlsHandshakeFailed",
            TcpConnectError::UpstreamTlsHandshakeTimeout => "UpstreamTlsHandshakeTimeout",
            TcpConnectError::UpstreamTlsHandshakeFailed(_) => "UpstreamTlsHandshakeFailed",
        }
    }
}

impl From<TcpConnectError> for ServerTaskError {
    fn from(e: TcpConnectError) -> Self {
        match e {
            TcpConnectError::MethodUnavailable => {
                ServerTaskError::ForbiddenByRule(ServerTaskForbiddenError::MethodUnavailable)
            }
            TcpConnectError::EscaperNotUsable => ServerTaskError::EscaperNotUsable,
            TcpConnectError::ResolveFailed(e) => ServerTaskError::from(e),
            TcpConnectError::SetupSocketFailed(_) => ServerTaskError::InternalServerError(
                "failed to setup local socket for remote connection",
            ),
            TcpConnectError::ConnectFailed(e) => ServerTaskError::UpstreamNotConnected(e),
            TcpConnectError::TimeoutByRule => {
                ServerTaskError::UpstreamNotConnected(ConnectError::TimedOut)
            }
            TcpConnectError::NoAddressConnected => ServerTaskError::UpstreamNotAvailable,
            TcpConnectError::ForbiddenAddressFamily | TcpConnectError::ForbiddenRemoteAddress => {
                ServerTaskError::ForbiddenByRule(ServerTaskForbiddenError::IpBlocked)
            }
            TcpConnectError::ProxyProtocolEncodeError(_) => {
                ServerTaskError::InternalServerError("proxy protocol encode failed")
            }
            TcpConnectError::ProxyProtocolWriteFailed(e) => ServerTaskError::UpstreamWriteFailed(e),
            TcpConnectError::NegotiationReadFailed(e) => ServerTaskError::UpstreamReadFailed(e),
            TcpConnectError::NegotiationWriteFailed(e) => ServerTaskError::UpstreamWriteFailed(e),
            TcpConnectError::NegotiationRejected(e) => ServerTaskError::UpstreamNotNegotiated(e),
            TcpConnectError::NegotiationPeerTimeout => {
                ServerTaskError::UpstreamAppTimeout("negotiation peer timeout")
            }
            TcpConnectError::NegotiationProtocolErr => {
                ServerTaskError::InvalidUpstreamProtocol("protocol negotiation with remote failed")
            }
            TcpConnectError::InternalServerError(s) => ServerTaskError::InternalServerError(s),
            TcpConnectError::InternalTlsClientError(e) => {
                ServerTaskError::InternalTlsClientError(e)
            }
            TcpConnectError::PeerTlsHandshakeTimeout
            | TcpConnectError::PeerTlsHandshakeFailed(_) => {
                ServerTaskError::InternalServerError("tls handshake with remote peer failed")
            }
            TcpConnectError::UpstreamTlsHandshakeTimeout => {
                ServerTaskError::UpstreamTlsHandshakeTimeout
            }
            TcpConnectError::UpstreamTlsHandshakeFailed(e) => {
                ServerTaskError::UpstreamTlsHandshakeFailed(e)
            }
        }
    }
}

impl From<SocksConnectError> for TcpConnectError {
    fn from(e: SocksConnectError) -> Self {
        match e {
            SocksConnectError::ReadFailed(e) => TcpConnectError::NegotiationReadFailed(e),
            SocksConnectError::WriteFailed(e) => TcpConnectError::NegotiationWriteFailed(e),
            SocksConnectError::NoAuthMethodAvailable => {
                TcpConnectError::NegotiationRejected("no auth method".to_string())
            }
            SocksConnectError::UnsupportedAuthVersion => TcpConnectError::NegotiationRejected(
                "auth protocol mismatch with remote proxy".to_string(),
            ),
            SocksConnectError::AuthFailed => {
                TcpConnectError::NegotiationRejected("auth failed with remote proxy".to_string())
            }
            SocksConnectError::InvalidProtocol(_) => TcpConnectError::NegotiationProtocolErr,
            SocksConnectError::PeerTimeout => TcpConnectError::NegotiationPeerTimeout,
            SocksConnectError::RequestFailed(s) => TcpConnectError::NegotiationRejected(s),
        }
    }
}

impl From<&TcpConnectError> for Socks5Reply {
    fn from(e: &TcpConnectError) -> Self {
        match e {
            TcpConnectError::MethodUnavailable
            | TcpConnectError::ForbiddenAddressFamily
            | TcpConnectError::ForbiddenRemoteAddress => Socks5Reply::ForbiddenByRule,
            TcpConnectError::ConnectFailed(e) => match e {
                ConnectError::ConnectionRefused | ConnectError::ConnectionReset => {
                    Socks5Reply::ConnectionRefused
                }
                ConnectError::NetworkUnreachable => Socks5Reply::NetworkUnreachable,
                ConnectError::HostUnreachable => Socks5Reply::HostUnreachable,
                ConnectError::TimedOut => Socks5Reply::ConnectionTimedOut,
                ConnectError::UnspecifiedError(_) => Socks5Reply::GeneralServerFailure,
            },
            TcpConnectError::ResolveFailed(_) | TcpConnectError::NoAddressConnected => {
                Socks5Reply::HostUnreachable
            }
            TcpConnectError::TimeoutByRule => Socks5Reply::ConnectionTimedOut,
            TcpConnectError::EscaperNotUsable
            | TcpConnectError::SetupSocketFailed(_)
            | TcpConnectError::ProxyProtocolEncodeError(_)
            | TcpConnectError::NegotiationProtocolErr => Socks5Reply::GeneralServerFailure,
            TcpConnectError::ProxyProtocolWriteFailed(_)
            | TcpConnectError::NegotiationReadFailed(_)
            | TcpConnectError::NegotiationWriteFailed(_) => Socks5Reply::GeneralServerFailure,
            TcpConnectError::NegotiationRejected(_) => Socks5Reply::ConnectionRefused,
            TcpConnectError::NegotiationPeerTimeout => Socks5Reply::ConnectionTimedOut,
            TcpConnectError::InternalServerError(_)
            | TcpConnectError::InternalTlsClientError(_) => Socks5Reply::GeneralServerFailure,
            TcpConnectError::PeerTlsHandshakeTimeout
            | TcpConnectError::PeerTlsHandshakeFailed(_) => Socks5Reply::GeneralServerFailure,
            TcpConnectError::UpstreamTlsHandshakeTimeout
            | TcpConnectError::UpstreamTlsHandshakeFailed(_) => Socks5Reply::GeneralServerFailure,
        }
    }
}

impl From<HttpConnectError> for TcpConnectError {
    fn from(e: HttpConnectError) -> Self {
        match e {
            HttpConnectError::RemoteClosed => TcpConnectError::NegotiationReadFailed(
                io::Error::new(io::ErrorKind::UnexpectedEof, "early eof"),
            ),
            HttpConnectError::ReadFailed(e) => TcpConnectError::NegotiationReadFailed(e),
            HttpConnectError::WriteFailed(e) => TcpConnectError::NegotiationWriteFailed(e),
            HttpConnectError::InvalidResponse(_) => TcpConnectError::NegotiationProtocolErr,
            HttpConnectError::UnexpectedStatusCode(code, reason) => {
                TcpConnectError::NegotiationRejected(format!(
                    "rejected by remote proxy with response {code} {reason}"
                ))
            }
            HttpConnectError::PeerTimeout(_) => TcpConnectError::NegotiationPeerTimeout,
        }
    }
}
