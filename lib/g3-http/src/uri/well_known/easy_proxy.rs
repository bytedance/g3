/*
 * SPDX-License-Identifier: Apache-2.0
 * Copyright 2025 ByteDance and/or its affiliates.
 */

use std::str::FromStr;

use bytes::Bytes;
use http::Uri;
use http::uri::{Authority, PathAndQuery, Scheme};

use g3_types::net::{Host, HttpProxySubProtocol, UpstreamAddr};

use super::{UriParseError, WellKnownUri, WellKnownUriParser};

impl WellKnownUriParser<'_> {
    pub(super) fn parse_easy_proxy(&mut self) -> Result<WellKnownUri, UriParseError> {
        let Some(scheme) = self.next_path_segment() else {
            return Err(UriParseError::RequiredFieldNotFound("scheme"));
        };
        let (protocol, scheme) = match scheme {
            "http" => (HttpProxySubProtocol::HttpForward, Scheme::HTTP),
            "https" => (HttpProxySubProtocol::HttpsForward, Scheme::HTTPS),
            "ftp" => (
                HttpProxySubProtocol::FtpOverHttp,
                Scheme::from_str("ftp").unwrap(),
            ),
            _ => return Err(UriParseError::NotValidScheme("scheme")),
        };

        let host = self
            .next_path_segment()
            .ok_or(UriParseError::RequiredFieldNotFound("target_host"))?;
        let host = Host::from_str(host).map_err(|_| UriParseError::NotValidHost("target_host"))?;

        let port = self
            .next_path_segment()
            .ok_or(UriParseError::RequiredFieldNotFound("target_port"))?;
        let port = u16::from_str(port).map_err(|_| UriParseError::NotValidPort("target_port"))?;

        let target = UpstreamAddr::new(host, port);
        let target_s = target.to_string();
        let authority = Authority::from_maybe_shared(Bytes::from(target_s))
            .map_err(|_| UriParseError::NotValidHost("host"))?;

        let pq = self.uri.path_and_query().unwrap().as_str();
        let left_pq = &pq[self.path_offset - 1..]; // should include the first '/'
        let path = PathAndQuery::from_str(left_pq).unwrap();

        let uri = Uri::builder()
            .scheme(scheme)
            .authority(authority)
            .path_and_query(path)
            .build()
            .unwrap();

        Ok(WellKnownUri::EasyProxy(protocol, target, uri))
    }
}
