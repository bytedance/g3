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

use std::io::{self, Write};
use std::net::{IpAddr, SocketAddr};

use ascii::AsciiStr;
use askama::Template;
use http::{StatusCode, Version};
use mime::Mime;
use tokio::io::{AsyncWrite, AsyncWriteExt, BufWriter};

use g3_ftp_client::FtpConnectError;
use g3_http::server::HttpRequestParseError;
use g3_types::net::ConnectError;

use crate::module::http_header;
use crate::module::tcp_connect::TcpConnectError;
use crate::serve::ServerTaskError;

#[derive(Template)]
#[template(path = "error.html")]
struct ErrorPageTemplate<'a> {
    code: u16,
    reason: &'a str,
}

struct CustomStatusCode {}

impl CustomStatusCode {
    const WEB_SERVER_IS_DOWN: u16 = 521;
    const CONNECTION_TIMED_OUT: u16 = 522;
    const ORIGIN_IS_UNREACHABLE: u16 = 523;
    const SSL_HANDSHAKE_FAILED: u16 = 525;
    const ORIGIN_DNS_ERROR: u16 = 530;

    fn canonical_reason(code: u16) -> &'static str {
        match code {
            Self::WEB_SERVER_IS_DOWN => "Web Server Is Down",
            Self::CONNECTION_TIMED_OUT => "Connection Timed Out",
            Self::ORIGIN_IS_UNREACHABLE => "Origin Is Unreachable",
            Self::ORIGIN_DNS_ERROR => "Origin DNS Error",
            Self::SSL_HANDSHAKE_FAILED => "SSL Handshake Failed",
            _ => "<unknown status code>",
        }
    }
}

pub(crate) struct HttpProxyClientResponse {
    status: StatusCode,
    version: Version,
    close: bool,
    extra_headers: Vec<String>,
}

impl HttpProxyClientResponse {
    const RESPONSE_BUFFER_SIZE: usize = 1024;

    pub(crate) fn status(&self) -> u16 {
        self.status.as_u16()
    }

    pub(crate) fn from_standard(status: StatusCode, version: Version, close: bool) -> Self {
        HttpProxyClientResponse {
            status,
            version,
            close,
            extra_headers: Vec::new(),
        }
    }

    pub(crate) fn add_extra_header(&mut self, line: String) {
        self.extra_headers.push(line);
    }

    pub(crate) fn set_upstream_addr(&mut self, addr: SocketAddr) {
        self.extra_headers.push(http_header::upstream_addr(addr));
    }

    pub(crate) fn set_outgoing_ip(&mut self, ip: IpAddr) {
        self.extra_headers.push(http_header::outgoing_ip(ip));
    }

    #[inline]
    pub(crate) fn too_many_requests(version: Version) -> Self {
        HttpProxyClientResponse::from_standard(StatusCode::TOO_MANY_REQUESTS, version, true)
    }

    #[inline]
    pub(crate) fn forbidden(version: Version) -> Self {
        HttpProxyClientResponse::from_standard(StatusCode::FORBIDDEN, version, true)
    }

    #[inline]
    pub(crate) fn method_not_allowed(version: Version) -> Self {
        HttpProxyClientResponse::from_standard(StatusCode::METHOD_NOT_ALLOWED, version, true)
    }

    #[allow(unused)]
    #[inline]
    pub(crate) fn unimplemented(version: Version) -> Self {
        HttpProxyClientResponse::from_standard(StatusCode::NOT_IMPLEMENTED, version, true)
    }

    #[inline]
    pub(crate) fn bad_request(version: Version) -> Self {
        HttpProxyClientResponse::from_standard(StatusCode::BAD_REQUEST, version, true)
    }

    #[inline]
    pub(crate) fn bad_gateway(version: Version) -> Self {
        HttpProxyClientResponse::from_standard(StatusCode::BAD_GATEWAY, version, true)
    }

    #[inline]
    pub(crate) fn service_unavailable(version: Version) -> Self {
        HttpProxyClientResponse::from_standard(StatusCode::SERVICE_UNAVAILABLE, version, true)
    }

    #[inline]
    pub(crate) fn resource_not_found(version: Version, close: bool) -> Self {
        HttpProxyClientResponse::from_standard(StatusCode::NOT_FOUND, version, close)
    }

    pub(crate) fn need_login(version: Version, close: bool, realm: &str) -> Self {
        let mut response =
            HttpProxyClientResponse::from_standard(StatusCode::UNAUTHORIZED, version, close);
        let auth_header = g3_http::header::www_authenticate_basic(realm);
        response.add_extra_header(auth_header);
        response
    }

    pub(crate) fn auto_chunked_ok(
        version: Version,
        close: bool,
        content_type: &Mime,
    ) -> (Self, bool) {
        let chunked: bool;
        let mut response = HttpProxyClientResponse::from_standard(StatusCode::OK, version, close);
        if close {
            chunked = false;
        } else if matches!(version, Version::HTTP_09 | Version::HTTP_10) {
            response.close = true;
            chunked = false;
        } else {
            response.add_extra_header(g3_http::header::transfer_encoding_chunked().to_string());
            chunked = true;
        }
        response.add_extra_header(g3_http::header::content_type(content_type));
        (response, chunked)
    }

    pub(crate) fn sized_ok(
        version: Version,
        close: bool,
        body_len: u64,
        content_type: &Mime,
    ) -> Self {
        let mut response = HttpProxyClientResponse::from_standard(StatusCode::OK, version, close);
        response.add_extra_header(g3_http::header::content_length(body_len));
        response.add_extra_header(g3_http::header::content_type(content_type));
        response
    }

    pub(crate) fn ending_ok(version: Version, close: bool, content_type: &Mime) -> Self {
        let mut response = HttpProxyClientResponse::from_standard(StatusCode::OK, version, close);
        response.add_extra_header(g3_http::header::content_type(content_type));
        response
    }

    pub(crate) fn ok(version: Version, close: bool) -> Self {
        let mut response = HttpProxyClientResponse::from_standard(StatusCode::OK, version, close);
        response.add_extra_header(g3_http::header::content_length(0));
        response
    }

    pub(crate) fn sized_partial_content(
        version: Version,
        close: bool,
        start_size: u64,
        end_size: u64,
        total_size: u64,
        content_type: &Mime,
    ) -> Self {
        let mut response =
            HttpProxyClientResponse::from_standard(StatusCode::PARTIAL_CONTENT, version, close);
        response.add_extra_header(g3_http::header::content_range_sized(
            start_size, end_size, total_size,
        ));
        response.add_extra_header(g3_http::header::content_length(end_size - start_size + 1));
        response.add_extra_header(g3_http::header::content_type(content_type));
        response
    }

    pub(crate) fn range_not_satisfiable(
        version: Version,
        close: bool,
        start_size: Option<u64>,
    ) -> Self {
        let mut response = HttpProxyClientResponse::from_standard(
            StatusCode::RANGE_NOT_SATISFIABLE,
            version,
            close,
        );
        if let Some(start) = start_size {
            response.add_extra_header(g3_http::header::content_range_overflowed(start));
        }
        response
    }

    pub(crate) fn from_request_error(e: &HttpRequestParseError, version: Version) -> Option<Self> {
        e.status_code()
            .map(|status| HttpProxyClientResponse::from_standard(status, version, true))
    }

    pub(crate) fn from_ftp_connect_error(
        e: &FtpConnectError<TcpConnectError>,
        version: Version,
        should_close: bool,
    ) -> Self {
        match e {
            FtpConnectError::ConnectIoError(e) => {
                HttpProxyClientResponse::from_tcp_connect_error(e, version, should_close)
            }
            FtpConnectError::ConnectTimedOut | FtpConnectError::GreetingTimedOut => {
                HttpProxyClientResponse::from_standard(StatusCode::GATEWAY_TIMEOUT, version, true)
            }
            FtpConnectError::GreetingFailed(_)
            | FtpConnectError::NegotiationFailed(_)
            | FtpConnectError::InvalidReplyCode(_) => {
                HttpProxyClientResponse::from_standard(StatusCode::BAD_GATEWAY, version, true)
            }
            FtpConnectError::ServiceNotAvailable => HttpProxyClientResponse::from_standard(
                StatusCode::SERVICE_UNAVAILABLE,
                version,
                true,
            ),
        }
    }

    pub(crate) fn from_tcp_connect_error(
        e: &TcpConnectError,
        version: Version,
        should_close: bool,
    ) -> Self {
        let close = should_close;
        match e {
            TcpConnectError::MethodUnavailable => {
                HttpProxyClientResponse::from_standard(StatusCode::FORBIDDEN, version, true)
            }
            TcpConnectError::EscaperNotUsable => HttpProxyClientResponse::from_standard(
                StatusCode::SERVICE_UNAVAILABLE,
                version,
                true,
            ),
            TcpConnectError::ResolveFailed(_) => HttpProxyClientResponse::from_standard(
                StatusCode::from_u16(CustomStatusCode::ORIGIN_DNS_ERROR).unwrap(),
                version,
                close,
            ),
            TcpConnectError::SetupSocketFailed(_) => HttpProxyClientResponse::from_standard(
                StatusCode::INTERNAL_SERVER_ERROR,
                version,
                true,
            ),
            TcpConnectError::ConnectFailed(e) => {
                HttpProxyClientResponse::from_net_connect_err(e, version, should_close)
            }
            TcpConnectError::TimeoutByRule => {
                HttpProxyClientResponse::from_standard(StatusCode::GATEWAY_TIMEOUT, version, close)
            }
            TcpConnectError::NoAddressConnected => {
                HttpProxyClientResponse::from_standard(StatusCode::BAD_GATEWAY, version, close)
            }
            TcpConnectError::ForbiddenAddressFamily | TcpConnectError::ForbiddenRemoteAddress => {
                HttpProxyClientResponse::from_standard(StatusCode::FORBIDDEN, version, close)
            }
            TcpConnectError::ProxyProtocolEncodeError(_) => HttpProxyClientResponse::from_standard(
                StatusCode::INTERNAL_SERVER_ERROR,
                version,
                true,
            ),
            TcpConnectError::ProxyProtocolWriteFailed(_)
            | TcpConnectError::NegotiationReadFailed(_)
            | TcpConnectError::NegotiationWriteFailed(_)
            | TcpConnectError::NegotiationRejected(_) => {
                HttpProxyClientResponse::from_standard(StatusCode::BAD_GATEWAY, version, true)
            }
            TcpConnectError::NegotiationPeerTimeout => {
                HttpProxyClientResponse::from_standard(StatusCode::GATEWAY_TIMEOUT, version, close)
            }
            TcpConnectError::NegotiationProtocolErr => {
                HttpProxyClientResponse::from_standard(StatusCode::BAD_GATEWAY, version, true)
            }
            TcpConnectError::InternalServerError(_)
            | TcpConnectError::InternalTlsClientError(_) => HttpProxyClientResponse::from_standard(
                StatusCode::INTERNAL_SERVER_ERROR,
                version,
                true,
            ),
            TcpConnectError::PeerTlsHandshakeTimeout
            | TcpConnectError::PeerTlsHandshakeFailed(_) => HttpProxyClientResponse::from_standard(
                StatusCode::INTERNAL_SERVER_ERROR,
                version,
                true,
            ),
            TcpConnectError::UpstreamTlsHandshakeTimeout
            | TcpConnectError::UpstreamTlsHandshakeFailed(_) => {
                HttpProxyClientResponse::from_standard(
                    StatusCode::from_u16(CustomStatusCode::SSL_HANDSHAKE_FAILED).unwrap(),
                    version,
                    close,
                )
            }
        }
    }

    pub(crate) fn from_task_err(
        e: &ServerTaskError,
        version: Version,
        should_close: bool,
    ) -> Option<Self> {
        let close = should_close; // no retry on the same connection if there's body pending
        let r = match e {
            ServerTaskError::InternalServerError(_)
            | ServerTaskError::InternalAdapterError(_)
            | ServerTaskError::InternalResolverError(_)
            | ServerTaskError::UnclassifiedError(_) => HttpProxyClientResponse::from_standard(
                StatusCode::INTERNAL_SERVER_ERROR,
                version,
                close,
            ),
            ServerTaskError::InternalTlsClientError(_) => HttpProxyClientResponse::from_standard(
                StatusCode::INTERNAL_SERVER_ERROR,
                version,
                true,
            ),
            ServerTaskError::EscaperNotUsable => HttpProxyClientResponse::from_standard(
                StatusCode::SERVICE_UNAVAILABLE,
                version,
                true,
            ),
            ServerTaskError::ForbiddenByRule(_) => {
                HttpProxyClientResponse::from_standard(StatusCode::FORBIDDEN, version, true)
            }
            ServerTaskError::InvalidClientProtocol(_) => {
                HttpProxyClientResponse::from_standard(StatusCode::BAD_REQUEST, version, true)
            }
            ServerTaskError::UnimplementedProtocol => {
                HttpProxyClientResponse::from_standard(StatusCode::NOT_IMPLEMENTED, version, true)
            }
            ServerTaskError::ClientAuthFailed => {
                // not in this stage
                return None;
            }
            ServerTaskError::UpstreamNotResolved(_) => HttpProxyClientResponse::from_standard(
                StatusCode::from_u16(CustomStatusCode::ORIGIN_DNS_ERROR).unwrap(),
                version,
                close,
            ),
            ServerTaskError::UpstreamNotConnected(e) => {
                Self::from_net_connect_err(e, version, should_close)
            }
            ServerTaskError::UpstreamNotAvailable => {
                HttpProxyClientResponse::from_standard(StatusCode::BAD_GATEWAY, version, close)
            }
            ServerTaskError::InvalidUpstreamProtocol(_) => {
                HttpProxyClientResponse::from_standard(StatusCode::BAD_GATEWAY, version, true)
            }
            ServerTaskError::UpstreamReadFailed(_)
            | ServerTaskError::UpstreamWriteFailed(_)
            | ServerTaskError::UpstreamNotNegotiated(_)
            | ServerTaskError::UpstreamAppError(_)
            | ServerTaskError::ClosedByUpstream => {
                HttpProxyClientResponse::from_standard(StatusCode::BAD_GATEWAY, version, true)
            }
            ServerTaskError::UpstreamTlsHandshakeTimeout
            | ServerTaskError::UpstreamTlsHandshakeFailed(_) => {
                HttpProxyClientResponse::from_standard(
                    StatusCode::from_u16(CustomStatusCode::SSL_HANDSHAKE_FAILED).unwrap(),
                    version,
                    close,
                )
            }
            ServerTaskError::UpstreamAppUnavailable => HttpProxyClientResponse::from_standard(
                StatusCode::SERVICE_UNAVAILABLE,
                version,
                true,
            ),
            ServerTaskError::UpstreamAppTimeout(_) => {
                HttpProxyClientResponse::from_standard(StatusCode::GATEWAY_TIMEOUT, version, true)
            }
            ServerTaskError::ClientAppTimeout(_) => {
                HttpProxyClientResponse::from_standard(StatusCode::REQUEST_TIMEOUT, version, true)
            }
            ServerTaskError::CanceledAsUserBlocked => {
                HttpProxyClientResponse::from_standard(StatusCode::FORBIDDEN, version, true)
            }
            ServerTaskError::CanceledAsServerQuit => HttpProxyClientResponse::from_standard(
                StatusCode::INTERNAL_SERVER_ERROR,
                version,
                true,
            ),
            ServerTaskError::ClientTcpReadFailed(_)
            | ServerTaskError::ClientTcpWriteFailed(_)
            | ServerTaskError::ClientUdpRecvFailed(_)
            | ServerTaskError::ClientUdpSendFailed(_)
            | ServerTaskError::ClosedByClient
            | ServerTaskError::ClosedEarlyByClient
            | ServerTaskError::Idle(_, _)
            | ServerTaskError::InterceptionError(_, _)
            | ServerTaskError::Finished => return None,
        };
        Some(r)
    }

    fn from_net_connect_err(e: &ConnectError, version: Version, should_close: bool) -> Self {
        let close = should_close;
        match e {
            ConnectError::ConnectionRefused | ConnectError::ConnectionReset => {
                HttpProxyClientResponse::from_standard(
                    StatusCode::from_u16(CustomStatusCode::WEB_SERVER_IS_DOWN).unwrap(),
                    version,
                    close,
                )
            }
            ConnectError::NetworkUnreachable | ConnectError::HostUnreachable => {
                HttpProxyClientResponse::from_standard(
                    StatusCode::from_u16(CustomStatusCode::ORIGIN_IS_UNREACHABLE).unwrap(),
                    version,
                    close,
                )
            }
            ConnectError::TimedOut => HttpProxyClientResponse::from_standard(
                StatusCode::from_u16(CustomStatusCode::CONNECTION_TIMED_OUT).unwrap(),
                version,
                close,
            ),
            ConnectError::UnspecifiedError(_) => {
                HttpProxyClientResponse::from_standard(StatusCode::BAD_GATEWAY, version, close)
            }
        }
    }

    pub(crate) fn should_close(&self) -> bool {
        self.close
    }

    fn canonical_reason(&self) -> &'static str {
        let code = self.status.as_u16();
        self.status
            .canonical_reason()
            .unwrap_or_else(|| CustomStatusCode::canonical_reason(code))
    }

    pub(crate) async fn reply_ok_to_connect<W>(&self, writer: &mut W) -> io::Result<()>
    where
        W: AsyncWrite + Unpin,
    {
        let mut header = Vec::<u8>::with_capacity(Self::RESPONSE_BUFFER_SIZE);
        write!(
            header,
            "{:?} {} {}\r\n",
            self.version,
            self.status.as_str(),
            self.canonical_reason(),
        )?;
        for line in &self.extra_headers {
            header.extend_from_slice(line.as_bytes());
        }
        header.extend_from_slice(b"\r\n");
        writer.write_all(header.as_ref()).await?;
        writer.flush().await?;
        Ok(())
    }

    pub(crate) async fn reply_ok_header<W>(&self, writer: &mut W) -> io::Result<()>
    where
        W: AsyncWrite + Unpin,
    {
        let mut header = Vec::<u8>::with_capacity(Self::RESPONSE_BUFFER_SIZE);
        write!(
            header,
            "{:?} {} {}\r\n",
            self.version,
            self.status.as_str(),
            self.canonical_reason(),
        )?;
        for line in &self.extra_headers {
            header.extend_from_slice(line.as_bytes());
        }
        header.extend_from_slice(g3_http::header::connection_as_bytes(self.close));
        header.extend_from_slice(b"\r\n");
        writer.write_all(header.as_ref()).await?;
        // writer.flush().await?;
        Ok(())
    }

    pub(crate) async fn reply_continue<W>(version: Version, writer: &mut W) -> io::Result<()>
    where
        W: AsyncWrite + Unpin,
    {
        let s = format!("{version:?} 100 Continue\r\n\r\n");
        writer.write_all(s.as_bytes()).await?;
        writer.flush().await?;
        Ok(())
    }

    async fn reply_err<W>(&self, writer: &mut W) -> io::Result<()>
    where
        W: AsyncWrite + Unpin,
    {
        let mut writer = BufWriter::new(writer);

        let error = ErrorPageTemplate {
            code: self.status.as_u16(),
            reason: self.canonical_reason(),
        };
        let body = error
            .render()
            .map_or_else(|e| format!("unable to render http body: {e}"), |v| v);

        let mut header = Vec::<u8>::with_capacity(Self::RESPONSE_BUFFER_SIZE);
        write!(
            header,
            "{:?} {} {}\r\n",
            self.version,
            self.status.as_str(),
            error.reason,
        )?;
        for line in &self.extra_headers {
            header.extend_from_slice(line.as_bytes());
        }
        header.extend_from_slice(g3_http::header::content_type(&mime::TEXT_HTML).as_bytes());
        header.extend_from_slice(g3_http::header::content_length(body.len() as u64).as_bytes());
        header.extend_from_slice(g3_http::header::connection_as_bytes(self.close));
        header.extend_from_slice(b"\r\n");

        writer.write_all(header.as_ref()).await?;
        writer.write_all(body.as_bytes()).await?;
        writer.flush().await?;
        Ok(())
    }

    pub(crate) async fn reply_err_to_request<W>(&self, writer: &mut W) -> io::Result<()>
    where
        W: AsyncWrite + Unpin,
    {
        self.reply_err(writer).await
    }

    pub(crate) async fn reply_proxy_auth_err<W>(
        version: Version,
        writer: &mut W,
        realm: &AsciiStr,
        close: bool,
    ) -> io::Result<()>
    where
        W: AsyncWrite + Unpin,
    {
        let mut response = HttpProxyClientResponse::from_standard(
            StatusCode::PROXY_AUTHENTICATION_REQUIRED,
            version,
            close,
        );
        let auth_header = g3_http::header::proxy_authenticate_basic(realm.as_str());
        response.add_extra_header(auth_header);
        response.reply_err(writer).await
    }

    pub(crate) async fn reply_auth_err<W>(
        version: Version,
        writer: &mut W,
        realm: &AsciiStr,
        close: bool,
    ) -> io::Result<()>
    where
        W: AsyncWrite + Unpin,
    {
        let response = HttpProxyClientResponse::need_login(version, close, realm.as_str());
        response.reply_err(writer).await
    }
}
