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
use std::time::Duration;

use anyhow::anyhow;
use thiserror::Error;

use g3_dpi::Protocol;
use g3_ftp_client::FtpConnectError;
use g3_http::client::HttpResponseParseError;
use g3_http::server::HttpRequestParseError;
use g3_icap_client::reqmod::h1::H1ReqmodAdaptationError;
use g3_icap_client::respmod::h1::H1RespmodAdaptationError;
use g3_io_ext::{
    IdleForceQuitReason, UdpCopyClientError, UdpCopyError, UdpCopyRemoteError, UdpRelayClientError,
    UdpRelayError, UdpRelayRemoteError,
};
use g3_resolver::ResolveError;
use g3_socks::SocksRequestParseError;
use g3_types::net::ConnectError;

use crate::inspect::InterceptionError;
use crate::module::tcp_connect::TcpConnectError;

#[derive(Error, Debug)]
pub(crate) enum ServerTaskForbiddenError {
    #[error("method unavailable")]
    MethodUnavailable,
    #[error("client ip blocked")]
    ClientIpBlocked,
    #[error("request rate limited")]
    RateLimited,
    #[error("proxy request type banned")]
    ProtoBanned,
    #[error("target dest denied")]
    DestDenied,
    #[error("target ip blocked")]
    IpBlocked,
    #[error("fully loaded")]
    FullyLoaded,
    #[error("http ua blocked")]
    UaBlocked,
    #[error("user blocked")]
    UserBlocked,
}

#[derive(Error, Debug)]
pub(crate) enum ServerTaskError {
    #[error("internal server error: {0}")]
    InternalServerError(&'static str),
    #[error("internal adapter error: {0}")]
    InternalAdapterError(anyhow::Error),
    #[error("internal resolver error: {0}")]
    InternalResolverError(ResolveError),
    #[error("internal tls client error: {0}")]
    InternalTlsClientError(anyhow::Error),
    #[error("escaper not usable")]
    EscaperNotUsable,
    #[error("forbidden by rule: {0}")]
    ForbiddenByRule(#[from] ServerTaskForbiddenError),
    #[error("invalid client protocol: {0}")]
    InvalidClientProtocol(&'static str),
    #[error("unimplemented protocol")]
    UnimplementedProtocol,
    #[error("tcp read from client: {0:?}")]
    ClientTcpReadFailed(io::Error),
    #[error("tcp write to client: {0:?}")]
    ClientTcpWriteFailed(io::Error),
    #[error("udp recv from client: {0:?}")]
    ClientUdpRecvFailed(io::Error),
    #[error("udp send to client: {0:?}")]
    ClientUdpSendFailed(io::Error),
    #[error("client authentication failed")]
    ClientAuthFailed,
    #[error("client app timeout: {0}")]
    ClientAppTimeout(&'static str),
    #[error("upstream not resolved: {0}")]
    UpstreamNotResolved(ResolveError),
    #[error("upstream not connected: {0}")]
    UpstreamNotConnected(ConnectError),
    #[error("upstream not available")]
    UpstreamNotAvailable,
    #[error("invalid upstream protocol: {0}")]
    InvalidUpstreamProtocol(&'static str),
    #[error("read from upstream: {0:?}")]
    UpstreamReadFailed(io::Error),
    #[error("write to upstream: {0:?}")]
    UpstreamWriteFailed(io::Error),
    #[error("upstream tls handshake timeout")]
    UpstreamTlsHandshakeTimeout,
    #[error("upstream tls handshake failed: {0:?}")]
    UpstreamTlsHandshakeFailed(anyhow::Error),
    #[error("upstream not negotiated: {0}")]
    UpstreamNotNegotiated(String),
    #[error("upstream app unavailable")]
    UpstreamAppUnavailable,
    #[error("upstream app timeout: {0}")]
    UpstreamAppTimeout(&'static str),
    #[error("upstream app error: {0:?}")]
    UpstreamAppError(anyhow::Error), // may contain upstream app timeout error
    #[error("closed by upstream")]
    ClosedByUpstream,
    #[error("closed by client")]
    ClosedByClient,
    #[error("closed early by client")]
    ClosedEarlyByClient,
    #[error("canceled as user blocked")]
    CanceledAsUserBlocked,
    #[error("canceled as server quit")]
    CanceledAsServerQuit,
    #[error("idle after {0:?} x {1}")]
    Idle(Duration, i32),
    #[error("{0} interception error: {1}")]
    InterceptionError(Protocol, InterceptionError),
    #[error("finished")]
    Finished, // this isn't an error, for log only
    #[error("unclassified error: {0:?}")]
    UnclassifiedError(#[from] anyhow::Error),
}

impl ServerTaskError {
    pub(crate) fn brief(&self) -> &'static str {
        match self {
            ServerTaskError::InternalServerError(_) => "InternalServerError",
            ServerTaskError::InternalAdapterError(_) => "InternalAdapterError",
            ServerTaskError::InternalResolverError(_) => "InternalResolverError",
            ServerTaskError::InternalTlsClientError(_) => "InternalTlsClientError",
            ServerTaskError::EscaperNotUsable => "EscaperNotUsable",
            ServerTaskError::ForbiddenByRule(_) => "ForbiddenByRule",
            ServerTaskError::InvalidClientProtocol(_) => "InvalidClientProtocol",
            ServerTaskError::UnimplementedProtocol => "UnimplementedProtocol",
            ServerTaskError::ClientTcpReadFailed(_) => "ClientTcpReadFailed",
            ServerTaskError::ClientTcpWriteFailed(_) => "ClientTcpWriteFailed",
            ServerTaskError::ClientUdpRecvFailed(_) => "ClientUdpRecvFailed",
            ServerTaskError::ClientUdpSendFailed(_) => "ClientUdpSendFailed",
            ServerTaskError::ClientAuthFailed => "ClientAuthFailed",
            ServerTaskError::ClientAppTimeout(_) => "ClientAppTimeout",
            ServerTaskError::UpstreamNotResolved(_) => "UpstreamNotResolved",
            ServerTaskError::UpstreamNotConnected(_) => "UpstreamNotConnected",
            ServerTaskError::UpstreamNotAvailable => "UpstreamNotAvailable",
            ServerTaskError::InvalidUpstreamProtocol(_) => "InvalidUpstreamProtocol",
            ServerTaskError::UpstreamReadFailed(_) => "UpstreamReadFailed",
            ServerTaskError::UpstreamWriteFailed(_) => "UpstreamWriteFailed",
            ServerTaskError::UpstreamTlsHandshakeTimeout => "UpstreamTlsHandshakeTimeout",
            ServerTaskError::UpstreamTlsHandshakeFailed(_) => "UpstreamTlsHandshakeFailed",
            ServerTaskError::UpstreamNotNegotiated(_) => "UpstreamNotNegotiated",
            ServerTaskError::UpstreamAppUnavailable => "UpstreamAppUnavailable",
            ServerTaskError::UpstreamAppTimeout(_) => "UpstreamAppTimeout",
            ServerTaskError::UpstreamAppError(_) => "UpstreamAppError",
            ServerTaskError::ClosedByUpstream => "ClosedByUpstream",
            ServerTaskError::ClosedByClient => "ClosedByClient",
            ServerTaskError::ClosedEarlyByClient => "ClosedEarlyByClient",
            ServerTaskError::CanceledAsUserBlocked => "CanceledAsUserBlocked",
            ServerTaskError::CanceledAsServerQuit => "CanceledAsServerQuit",
            ServerTaskError::Idle(_, _) => "Idle",
            ServerTaskError::InterceptionError(_, _) => "InterceptionError",
            ServerTaskError::Finished => "Finished",
            ServerTaskError::UnclassifiedError(_) => "UnclassifiedError",
        }
    }
}

pub(crate) type ServerTaskResult<T> = Result<T, ServerTaskError>;

impl From<ResolveError> for ServerTaskError {
    fn from(e: ResolveError) -> Self {
        if matches!(e, ResolveError::FromServer(_)) {
            ServerTaskError::UpstreamNotResolved(e)
        } else {
            ServerTaskError::InternalResolverError(e)
        }
    }
}

impl From<UdpRelayClientError> for ServerTaskError {
    fn from(e: UdpRelayClientError) -> Self {
        match e {
            UdpRelayClientError::RecvFailed(e) => ServerTaskError::ClientUdpRecvFailed(e),
            UdpRelayClientError::SendFailed(e) => ServerTaskError::ClientUdpSendFailed(e),
            UdpRelayClientError::InvalidPacket(_) => {
                ServerTaskError::InvalidClientProtocol("invalid udp packet from client")
            }
            UdpRelayClientError::AddressNotSupported => ServerTaskError::UnimplementedProtocol,
            UdpRelayClientError::MismatchedClientAddress
            | UdpRelayClientError::ForbiddenClientAddress => {
                ServerTaskError::ForbiddenByRule(ServerTaskForbiddenError::ClientIpBlocked)
            }
            UdpRelayClientError::ForbiddenTargetAddress => {
                ServerTaskError::ForbiddenByRule(ServerTaskForbiddenError::DestDenied)
            }
        }
    }
}

impl From<UdpRelayRemoteError> for ServerTaskError {
    fn from(e: UdpRelayRemoteError) -> Self {
        match e {
            UdpRelayRemoteError::NoListenSocket => {
                ServerTaskError::InternalServerError("no running udp listen socket at remote side")
            }
            UdpRelayRemoteError::RecvFailed(_, e) => ServerTaskError::UpstreamReadFailed(e),
            UdpRelayRemoteError::SendFailed(_, _, e) => ServerTaskError::UpstreamWriteFailed(e),
            UdpRelayRemoteError::InvalidPacket(_, _) => {
                ServerTaskError::InvalidUpstreamProtocol("invalid received udp packet")
            }
            UdpRelayRemoteError::AddressNotSupported => ServerTaskError::UnimplementedProtocol,
            UdpRelayRemoteError::DomainNotResolved(e) => ServerTaskError::from(e),
            UdpRelayRemoteError::ForbiddenTargetIpAddress(_) => {
                ServerTaskError::ForbiddenByRule(ServerTaskForbiddenError::IpBlocked)
            }
            UdpRelayRemoteError::RemoteSessionClosed(_, _) => ServerTaskError::ClosedByUpstream,
            UdpRelayRemoteError::RemoteSessionError(_, _, e) => {
                ServerTaskError::UpstreamReadFailed(e)
            }
            UdpRelayRemoteError::InternalServerError(s) => ServerTaskError::InternalServerError(s),
        }
    }
}

impl From<UdpRelayError> for ServerTaskError {
    fn from(e: UdpRelayError) -> Self {
        match e {
            UdpRelayError::ClientError(e) => ServerTaskError::from(e),
            UdpRelayError::RemoteError(_, e) => ServerTaskError::from(e),
        }
    }
}

impl From<UdpCopyClientError> for ServerTaskError {
    fn from(e: UdpCopyClientError) -> Self {
        match e {
            UdpCopyClientError::RecvFailed(e) => ServerTaskError::ClientUdpRecvFailed(e),
            UdpCopyClientError::SendFailed(e) => ServerTaskError::ClientUdpSendFailed(e),
            UdpCopyClientError::InvalidPacket(_) => {
                ServerTaskError::InvalidClientProtocol("invalid udp packet from client")
            }
            UdpCopyClientError::MismatchedClientAddress
            | UdpCopyClientError::ForbiddenClientAddress => {
                ServerTaskError::ForbiddenByRule(ServerTaskForbiddenError::ClientIpBlocked)
            }
            UdpCopyClientError::VaryUpstream => {
                ServerTaskError::InvalidClientProtocol("vary upstream for udp connect")
            }
        }
    }
}

impl From<UdpCopyRemoteError> for ServerTaskError {
    fn from(e: UdpCopyRemoteError) -> Self {
        match e {
            UdpCopyRemoteError::RecvFailed(e) => ServerTaskError::UpstreamReadFailed(e),
            UdpCopyRemoteError::SendFailed(e) => ServerTaskError::UpstreamWriteFailed(e),
            UdpCopyRemoteError::InvalidPacket(_) => {
                ServerTaskError::InvalidUpstreamProtocol("invalid received udp packet")
            }
            UdpCopyRemoteError::RemoteSessionClosed => ServerTaskError::ClosedByUpstream,
            UdpCopyRemoteError::RemoteSessionError(e) => ServerTaskError::UpstreamReadFailed(e),
            UdpCopyRemoteError::InternalServerError(s) => ServerTaskError::InternalServerError(s),
        }
    }
}

impl From<UdpCopyError> for ServerTaskError {
    fn from(e: UdpCopyError) -> Self {
        match e {
            UdpCopyError::ClientError(e) => ServerTaskError::from(e),
            UdpCopyError::RemoteError(e) => ServerTaskError::from(e),
        }
    }
}

impl From<HttpRequestParseError> for ServerTaskError {
    fn from(e: HttpRequestParseError) -> ServerTaskError {
        match e {
            HttpRequestParseError::ClientClosed => ServerTaskError::ClosedEarlyByClient,
            HttpRequestParseError::TooLargeHeader(_) => {
                ServerTaskError::InvalidClientProtocol("too large header in client request")
            }
            HttpRequestParseError::UpgradeIsNotSupported
            | HttpRequestParseError::UnsupportedMethod(_)
            | HttpRequestParseError::UnsupportedScheme => ServerTaskError::UnimplementedProtocol,
            HttpRequestParseError::IoFailed(e) => ServerTaskError::ClientTcpReadFailed(e),
            HttpRequestParseError::UnmatchedHostAndAuthority => {
                ServerTaskError::InvalidClientProtocol("host header doesn't match host in uri")
            }
            _ => ServerTaskError::InvalidClientProtocol("invalid client request"),
        }
    }
}

impl From<HttpResponseParseError> for ServerTaskError {
    fn from(e: HttpResponseParseError) -> ServerTaskError {
        match e {
            HttpResponseParseError::RemoteClosed => ServerTaskError::ClosedByUpstream,
            HttpResponseParseError::TooLargeHeader(_) => {
                ServerTaskError::InvalidUpstreamProtocol("too large header in remote response")
            }
            HttpResponseParseError::IoFailed(e) => ServerTaskError::UpstreamReadFailed(e),
            _ => ServerTaskError::InvalidUpstreamProtocol("invalid remote response"),
        }
    }
}

impl From<FtpConnectError<TcpConnectError>> for ServerTaskError {
    fn from(e: FtpConnectError<TcpConnectError>) -> Self {
        match e {
            FtpConnectError::ConnectIoError(e) => ServerTaskError::from(e),
            FtpConnectError::ConnectTimedOut => {
                ServerTaskError::UpstreamAppTimeout("ftp connect timed out")
            }
            FtpConnectError::GreetingTimedOut => {
                ServerTaskError::UpstreamAppTimeout("ftp greeting timed out")
            }
            FtpConnectError::GreetingFailed(_)
            | FtpConnectError::NegotiationFailed(_)
            | FtpConnectError::InvalidReplyCode(_)
            | FtpConnectError::ServiceNotAvailable => {
                ServerTaskError::UpstreamNotNegotiated(format!("ftp connect failed: {e}"))
            }
        }
    }
}

impl From<SocksRequestParseError> for ServerTaskError {
    fn from(e: SocksRequestParseError) -> Self {
        match e {
            SocksRequestParseError::ReadFailed(e) => ServerTaskError::ClientTcpReadFailed(e),
            SocksRequestParseError::InvalidProtocol(_) => {
                ServerTaskError::InvalidClientProtocol("invalid socks protocol")
            }
            SocksRequestParseError::InvalidUdpPeerAddress => {
                ServerTaskError::InvalidClientProtocol(
                    "invalid udp peer address in negotiation stage",
                )
            }
            SocksRequestParseError::ClientClosed => ServerTaskError::ClosedEarlyByClient,
        }
    }
}

impl From<H1ReqmodAdaptationError> for ServerTaskError {
    fn from(e: H1ReqmodAdaptationError) -> Self {
        match e {
            H1ReqmodAdaptationError::InternalServerError(s) => {
                ServerTaskError::InternalServerError(s)
            }
            H1ReqmodAdaptationError::HttpClientReadFailed(e) => {
                ServerTaskError::ClientTcpReadFailed(e)
            }
            H1ReqmodAdaptationError::InvalidHttpClientRequestBody => {
                ServerTaskError::InvalidClientProtocol("invalid http body in client request")
            }
            H1ReqmodAdaptationError::HttpUpstreamWriteFailed(e) => {
                ServerTaskError::UpstreamWriteFailed(e)
            }
            H1ReqmodAdaptationError::HttpClientReadIdle => {
                ServerTaskError::ClientAppTimeout("idle while reading")
            }
            H1ReqmodAdaptationError::HttpUpstreamWriteIdle => {
                ServerTaskError::UpstreamAppTimeout("idle while writing")
            }
            H1ReqmodAdaptationError::IdleForceQuit(reason) => match reason {
                IdleForceQuitReason::UserBlocked => ServerTaskError::CanceledAsUserBlocked,
                IdleForceQuitReason::ServerQuit => ServerTaskError::CanceledAsServerQuit,
            },
            e => ServerTaskError::InternalAdapterError(anyhow!("reqmod: {e}")),
        }
    }
}

impl From<H1RespmodAdaptationError> for ServerTaskError {
    fn from(e: H1RespmodAdaptationError) -> Self {
        match e {
            H1RespmodAdaptationError::InternalServerError(s) => {
                ServerTaskError::InternalServerError(s)
            }
            H1RespmodAdaptationError::HttpUpstreamReadFailed(e) => {
                ServerTaskError::UpstreamReadFailed(e)
            }
            H1RespmodAdaptationError::InvalidHttpUpstreamResponseBody => {
                ServerTaskError::InvalidUpstreamProtocol("invalid http body in upstream response")
            }
            H1RespmodAdaptationError::HttpClientWriteFailed(e) => {
                ServerTaskError::ClientTcpWriteFailed(e)
            }
            H1RespmodAdaptationError::HttpUpstreamReadIdle => {
                ServerTaskError::UpstreamAppTimeout("idle while reading")
            }
            H1RespmodAdaptationError::HttpClientWriteIdle => {
                ServerTaskError::ClientAppTimeout("idle while writing")
            }
            H1RespmodAdaptationError::IdleForceQuit(reason) => match reason {
                IdleForceQuitReason::UserBlocked => ServerTaskError::CanceledAsUserBlocked,
                IdleForceQuitReason::ServerQuit => ServerTaskError::CanceledAsServerQuit,
            },
            e => ServerTaskError::InternalAdapterError(anyhow!("respmod: {e}")),
        }
    }
}
