/*
 * SPDX-License-Identifier: Apache-2.0
 * Copyright 2023-2025 ByteDance and/or its affiliates.
 */

use std::str::FromStr;

use http::Uri;

use g3_types::net::{HttpProxySubProtocol, UpstreamAddr};

use super::HttpRequestParseError;

pub trait UriExt {
    fn get_upstream_and_protocol(
        &self,
    ) -> Result<(UpstreamAddr, HttpProxySubProtocol), HttpRequestParseError>;

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
    fn get_upstream_and_protocol(
        &self,
    ) -> Result<(UpstreamAddr, HttpProxySubProtocol), HttpRequestParseError> {
        if let Some(scheme) = self.scheme() {
            if scheme.eq(&http::uri::Scheme::HTTP) {
                let upstream = self.get_upstream_with_default_port(80)?;
                Ok((upstream, HttpProxySubProtocol::HttpForward))
            } else if scheme.eq(&http::uri::Scheme::HTTPS) {
                let upstream = self.get_upstream_with_default_port(443)?;
                Ok((upstream, HttpProxySubProtocol::HttpsForward))
            } else if scheme.as_str().eq_ignore_ascii_case("ftp") {
                let upstream = self.get_upstream_with_default_port(21)?;
                Ok((upstream, HttpProxySubProtocol::FtpOverHttp))
            } else {
                Err(HttpRequestParseError::UnsupportedScheme)
            }
        } else {
            Err(HttpRequestParseError::InvalidRequestTarget)
        }
    }

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
