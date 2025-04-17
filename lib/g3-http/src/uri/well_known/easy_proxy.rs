/*
 * Copyright 2025 ByteDance and/or its affiliates.
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

use std::str::FromStr;

use http::Uri;
use http::uri::PathAndQuery;

use g3_types::net::{Host, HttpProxySubProtocol, UpstreamAddr};

use super::{UriParseError, WellKnownUri, WellKnownUriParser};

impl WellKnownUriParser<'_> {
    pub(super) fn parse_easy_proxy(&mut self) -> Result<WellKnownUri, UriParseError> {
        let Some(scheme) = self.next_path_segment() else {
            return Err(UriParseError::RequiredFieldNotFound("scheme"));
        };
        let protocol = match scheme {
            "http" => HttpProxySubProtocol::HttpForward,
            "https" => HttpProxySubProtocol::HttpsForward,
            "ftp" => HttpProxySubProtocol::FtpOverHttp,
            _ => return Err(UriParseError::NotValidScheme("scheme")),
        };

        let Some(host) = self.next_path_segment() else {
            return Err(UriParseError::RequiredFieldNotFound("target_host"));
        };
        let host = Host::from_str(host).map_err(|_| UriParseError::NotValidHost("target_host"))?;

        let Some(port) = self.next_path_segment() else {
            return Err(UriParseError::RequiredFieldNotFound("target_port"));
        };
        let port = u16::from_str(port).map_err(|_| UriParseError::NotValidPort("target_port"))?;

        let pq = self.uri.path_and_query().unwrap().as_str();
        let left_pq = &pq[self.path_offset - 1..]; // should include the first '/'
        let path = PathAndQuery::from_str(left_pq).unwrap();

        let uri = Uri::builder().path_and_query(path).build().unwrap();

        Ok(WellKnownUri::EasyProxy(
            protocol,
            UpstreamAddr::new(host, port),
            uri,
        ))
    }
}
