/*
 * SPDX-License-Identifier: Apache-2.0
 * Copyright 2023-2025 ByteDance and/or its affiliates.
 */

use http::{Method, Version};
use tokio::io::AsyncRead;
use tokio::sync::mpsc;
use tokio::time::Instant;

use g3_http::server::{HttpProxyClientRequest, HttpRequestParseError, UriExt};
use g3_http::uri::WellKnownUri;
use g3_types::net::{HttpProxySubProtocol, UpstreamAddr};

use super::HttpClientReader;
use crate::config::server::http_proxy::HttpProxyServerConfig;

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
        config: &HttpProxyServerConfig,
        reader: &mut HttpClientReader<CDR>,
        sender: mpsc::Sender<Option<HttpClientReader<CDR>>>,
        version: &mut Version,
    ) -> Result<(Self, bool), HttpRequestParseError> {
        let time_accepted = Instant::now();

        let mut req = HttpProxyClientRequest::parse(
            reader,
            config.req_hdr_max_size,
            version,
            |req, name, header| {
                match name.as_str() {
                    "proxy-authorization" => return req.parse_header_authorization(header.value),
                    "proxy-connection" => {
                        // proxy-connection is not standard, but at least curl use it
                        return req.parse_header_connection(header);
                    }
                    "forwarded" | "x-forwarded-for" => {
                        if config.steal_forwarded_for {
                            return Ok(());
                        }
                    }
                    _ => {}
                }
                req.append_parsed_header(name, header)?;
                Ok(())
            },
        )
        .await?;
        let time_received = Instant::now();

        let (upstream, sub_protocol) = if matches!(&req.method, &Method::CONNECT) {
            let addr = req.uri.get_upstream_with_default_port(443)?;
            (addr, HttpProxySubProtocol::TcpConnect)
        } else if req.is_local_request(&config.local_server_names) {
            match WellKnownUri::parse(&req.uri).map_err(|e| {
                HttpRequestParseError::UnsupportedRequest(format!("invalid well-known uri: {e}",))
            })? {
                Some(WellKnownUri::EasyProxy(protocol, addr, uri)) => {
                    req.uri = uri;
                    req.set_host(&addr);
                    (addr, protocol)
                }
                Some(v) => {
                    return Err(HttpRequestParseError::UnsupportedRequest(format!(
                        "unsupported well-known uri suffix: {}",
                        v.suffix()
                    )));
                }
                None => {
                    return Err(HttpRequestParseError::UnsupportedRequest(
                        "unsupported local request uri".to_string(),
                    ));
                }
            }
        } else if let Some(scheme) = req.uri.scheme() {
            if scheme.eq(&http::uri::Scheme::HTTP) {
                let upstream = req.uri.get_upstream_with_default_port(80)?;
                (upstream, HttpProxySubProtocol::HttpForward)
            } else if scheme.eq(&http::uri::Scheme::HTTPS) {
                let upstream = req.uri.get_upstream_with_default_port(443)?;
                (upstream, HttpProxySubProtocol::HttpsForward)
            } else if scheme.as_str().eq_ignore_ascii_case("ftp") {
                let upstream = req.uri.get_upstream_with_default_port(21)?;
                (upstream, HttpProxySubProtocol::FtpOverHttp)
            } else {
                return Err(HttpRequestParseError::UnsupportedScheme);
            }
        } else {
            return Err(HttpRequestParseError::InvalidRequestTarget);
        };

        if !config.allow_custom_host {
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
