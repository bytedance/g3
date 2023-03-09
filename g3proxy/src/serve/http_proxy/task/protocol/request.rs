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

use http::{Method, Version};
use tokio::io::AsyncRead;
use tokio::sync::mpsc;
use tokio::time::Instant;

use g3_http::server::{HttpProxyClientRequest, HttpRequestParseError, UriExt};
use g3_types::net::UpstreamAddr;

use super::{HttpClientReader, HttpProxySubProtocol};

pub(crate) struct HttpProxyRequest<CDR> {
    pub(crate) client_protocol: HttpProxySubProtocol,
    pub(crate) inner: HttpProxyClientRequest,
    pub(crate) upstream: UpstreamAddr,
    pub(crate) time_accepted: Instant,
    pub(crate) time_received: Instant,
    pub(crate) body_reader: Option<HttpClientReader<CDR>>,
    pub(crate) stream_sender: mpsc::Sender<Option<HttpClientReader<CDR>>>,
}

impl<CDR> HttpProxyRequest<CDR>
where
    CDR: AsyncRead + Unpin,
{
    pub(crate) async fn parse(
        reader: &mut HttpClientReader<CDR>,
        sender: mpsc::Sender<Option<HttpClientReader<CDR>>>,
        max_header_size: usize,
        steal_forwarder_for: bool,
        allow_custom_host: bool,
        version: &mut Version,
    ) -> Result<(Self, bool), HttpRequestParseError> {
        let time_accepted = Instant::now();

        let req =
            HttpProxyClientRequest::parse(reader, max_header_size, version, &|req, name, value| {
                match name.as_str() {
                    "proxy-authorization" => return req.parse_header_authorization(value),
                    "proxy-connection" => {
                        // proxy-connection is not standard, but at least curl use it
                        return req.parse_header_connection(value);
                    }
                    "forwarded" | "x-forwarded-for" => {
                        if steal_forwarder_for {
                            return Ok(());
                        }
                    }
                    _ => {}
                }
                req.append_header(name, value)?;
                Ok(())
            })
            .await?;
        let time_received = Instant::now();

        let (upstream, sub_protocol) = if matches!(&req.method, &Method::CONNECT) {
            (
                get_connect_upstream(&req.uri)?,
                HttpProxySubProtocol::TcpConnect,
            )
        } else {
            get_forward_upstream_and_protocol(&req.uri)?
        };

        if !allow_custom_host {
            if let Some(host) = &req.host {
                if !host.host_eq(&upstream) {
                    return Err(HttpRequestParseError::UnmatchedHostAndAuthority);
                }
            }
        }

        let req = HttpProxyRequest {
            client_protocol: sub_protocol,
            inner: req,
            upstream,
            time_accepted,
            time_received,
            body_reader: None,
            stream_sender: sender,
        };

        match req.client_protocol {
            HttpProxySubProtocol::TcpConnect => {
                // just send to forward task, which will go into a connect task
                // reader should be sent
                return Ok((req, true));
            }
            HttpProxySubProtocol::FtpOverHttp => {}
            HttpProxySubProtocol::HttpForward | HttpProxySubProtocol::HttpsForward => {
                if req.inner.pipeline_safe() {
                    // reader should not be sent
                    return Ok((req, false));
                }
            }
        }

        // reader should be sent by default
        Ok((req, true))
    }
}

fn get_connect_upstream(uri: &http::Uri) -> Result<UpstreamAddr, HttpRequestParseError> {
    uri.get_upstream_with_default_port(443)
}

fn get_forward_upstream_and_protocol(
    uri: &http::Uri,
) -> Result<(UpstreamAddr, HttpProxySubProtocol), HttpRequestParseError> {
    match uri.scheme() {
        Some(scheme) => {
            if scheme.eq(&http::uri::Scheme::HTTP) {
                let upstream = uri.get_upstream_with_default_port(80)?;
                Ok((upstream, HttpProxySubProtocol::HttpForward))
            } else if scheme.eq(&http::uri::Scheme::HTTPS) {
                let upstream = uri.get_upstream_with_default_port(443)?;
                Ok((upstream, HttpProxySubProtocol::HttpsForward))
            } else if scheme.as_str().eq_ignore_ascii_case("ftp") {
                let upstream = uri.get_upstream_with_default_port(21)?;
                Ok((upstream, HttpProxySubProtocol::FtpOverHttp))
            } else {
                Err(HttpRequestParseError::UnsupportedScheme)
            }
        }
        None => Err(HttpRequestParseError::InvalidRequestTarget),
    }
}
