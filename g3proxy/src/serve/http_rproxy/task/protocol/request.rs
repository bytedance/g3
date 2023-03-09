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

use std::borrow::Cow;

use http::{HeaderValue, Method, Version};
use tokio::io::AsyncRead;
use tokio::sync::mpsc;
use tokio::time::Instant;

use g3_http::server::{HttpProxyClientRequest, HttpRequestParseError, UriExt};
use g3_types::net::{HttpServerId, UpstreamAddr};

use super::HttpClientReader;

pub(crate) struct HttpRProxyRequest<CDR> {
    pub(crate) inner: HttpProxyClientRequest,
    pub(crate) upstream: UpstreamAddr,
    pub(crate) time_accepted: Instant,
    pub(crate) time_received: Instant,
    pub(crate) body_reader: Option<HttpClientReader<CDR>>,
    pub(crate) stream_sender: mpsc::Sender<Option<HttpClientReader<CDR>>>,
}

impl<CDR> HttpRProxyRequest<CDR>
where
    CDR: AsyncRead + Unpin,
{
    pub(crate) async fn parse(
        reader: &mut HttpClientReader<CDR>,
        sender: mpsc::Sender<Option<HttpClientReader<CDR>>>,
        max_header_size: usize,
        server_id: Option<&HttpServerId>,
        version: &mut Version,
    ) -> Result<(Self, bool), HttpRequestParseError> {
        let time_accepted = Instant::now();

        let mut req =
            HttpProxyClientRequest::parse(reader, max_header_size, version, &|req, name, value| {
                if name.as_str() == "authorization" {
                    return req.parse_header_authorization(value);
                }
                req.append_header(name, value)?;
                Ok(())
            })
            .await?;
        let time_received = Instant::now();

        if matches!(&req.method, &Method::CONNECT) {
            return Err(HttpRequestParseError::UnsupportedMethod(
                "CONNECT".to_string(),
            ));
        }

        let upstream = if let Some(mut host) = req.host.clone() {
            if let Some(u) = get_upstream_from_uri(&req.uri)? {
                if !host.host_eq(&u) {
                    return Err(HttpRequestParseError::UnmatchedHostAndAuthority);
                }
                if host.port() == 0 {
                    host.set_port(u.port());
                }
            }
            host
        } else {
            return Err(HttpRequestParseError::MissedHost);
        };

        // check VIA
        let this_pseudonym = server_id
            .map(|id| Cow::Borrowed(id.as_str()))
            .unwrap_or_else(|| upstream.host_str());
        for h in req.end_to_end_headers.get_all(http::header::VIA) {
            if let Some(pseudonym) = h
                .as_bytes()
                .splitn(3, |c| c.is_ascii_whitespace())
                .filter(|s| !s.is_empty())
                .nth(1)
            {
                if pseudonym.eq(this_pseudonym.as_bytes()) {
                    return Err(HttpRequestParseError::LoopDetected);
                }
            }
        }
        // append VIA
        let via_value = format!("HTTP/{:?} {}", req.version, this_pseudonym);
        let v = unsafe { HeaderValue::from_maybe_shared_unchecked(via_value) };
        req.end_to_end_headers.append(http::header::VIA, v);

        let req = HttpRProxyRequest {
            inner: req,
            upstream,
            time_accepted,
            time_received,
            body_reader: None,
            stream_sender: sender,
        };

        if req.inner.pipeline_safe() {
            // reader should not be sent
            return Ok((req, false));
        }

        // reader should be sent by default
        Ok((req, true))
    }
}

fn get_upstream_from_uri(uri: &http::Uri) -> Result<Option<UpstreamAddr>, HttpRequestParseError> {
    match uri.scheme() {
        Some(scheme) => {
            if scheme.eq(&http::uri::Scheme::HTTP) {
                uri.get_optional_upstream_with_default_port(80)
            } else {
                Err(HttpRequestParseError::UnsupportedScheme)
            }
        }
        None => Ok(None),
    }
}
