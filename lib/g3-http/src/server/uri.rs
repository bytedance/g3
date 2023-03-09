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

use std::str::FromStr;

use http::Uri;

use g3_types::net::UpstreamAddr;

use super::HttpRequestParseError;

pub trait UriExt {
    fn get_upstream_with_default_port(
        &self,
        default_port: u16,
    ) -> Result<UpstreamAddr, HttpRequestParseError>;

    fn get_optional_upstream_with_default_port(
        &self,
        default_port: u16,
    ) -> Result<Option<UpstreamAddr>, HttpRequestParseError>;

    fn get_connect_udp_upstream(&self) -> Result<UpstreamAddr, HttpRequestParseError>;
}

impl UriExt for Uri {
    fn get_upstream_with_default_port(
        &self,
        default_port: u16,
    ) -> Result<UpstreamAddr, HttpRequestParseError> {
        match self.authority() {
            Some(authority) => {
                let host = authority.host();
                let port = authority.port_u16().unwrap_or(default_port);
                Ok(UpstreamAddr::from_host_str_and_port(host, port)
                    .map_err(|_| HttpRequestParseError::InvalidRequestTarget)?)
            }
            None => Err(HttpRequestParseError::InvalidRequestTarget),
        }
    }

    fn get_optional_upstream_with_default_port(
        &self,
        default_port: u16,
    ) -> Result<Option<UpstreamAddr>, HttpRequestParseError> {
        match self.authority() {
            Some(authority) => {
                let host = authority.host();
                let port = authority.port_u16().unwrap_or(default_port);
                let upstream = UpstreamAddr::from_host_str_and_port(host, port)
                    .map_err(|_| HttpRequestParseError::InvalidRequestTarget)?;
                Ok(Some(upstream))
            }
            None => Ok(None),
        }
    }

    fn get_connect_udp_upstream(&self) -> Result<UpstreamAddr, HttpRequestParseError> {
        const PREFIX: &str = "/.well-known/masque/udp/";

        if !self.path().starts_with(PREFIX) {
            return Err(HttpRequestParseError::InvalidRequestTarget);
        }

        let mut parts = self.path()[PREFIX.len()..].split('/');
        let host = parts
            .next()
            .ok_or(HttpRequestParseError::InvalidRequestTarget)?;
        let port = parts
            .next()
            .ok_or(HttpRequestParseError::InvalidRequestTarget)?;
        let port = u16::from_str(port).map_err(|_| HttpRequestParseError::InvalidRequestTarget)?;

        let upstream = UpstreamAddr::from_host_str_and_port(host, port)
            .map_err(|_| HttpRequestParseError::InvalidRequestTarget)?;
        Ok(upstream)
    }
}
