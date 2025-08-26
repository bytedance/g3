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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn get_upstream_and_protocol() {
        // HTTP
        let uri = Uri::from_static("http://example.com:8080/path");
        let result = uri.get_upstream_and_protocol().unwrap();

        assert_eq!(result.0.host_str(), "example.com");
        assert_eq!(result.0.port(), 8080);
        assert_eq!(result.1, HttpProxySubProtocol::HttpForward);

        // HTTPS
        let uri = Uri::from_static("https://example.com:8443/path");
        let result = uri.get_upstream_and_protocol().unwrap();

        assert_eq!(result.0.host_str(), "example.com");
        assert_eq!(result.0.port(), 8443);
        assert_eq!(result.1, HttpProxySubProtocol::HttpsForward);

        // FTP
        let uri = Uri::from_static("ftp://example.com:2121/path");
        let result = uri.get_upstream_and_protocol().unwrap();

        assert_eq!(result.0.host_str(), "example.com");
        assert_eq!(result.0.port(), 2121);
        assert_eq!(result.1, HttpProxySubProtocol::FtpOverHttp);

        // Unsupported scheme
        let uri = Uri::from_static("ws://example.com/path");
        let result = uri.get_upstream_and_protocol().unwrap_err();

        assert!(matches!(result, HttpRequestParseError::UnsupportedScheme));

        // Invalid request target
        let uri = Uri::from_static("/path");
        let result = uri.get_upstream_and_protocol().unwrap_err();

        assert!(matches!(
            result,
            HttpRequestParseError::InvalidRequestTarget
        ));
    }

    #[test]
    fn get_upstream_and_protocol_default_port() {
        // HTTP
        let uri = Uri::from_static("http://example.com/path");
        let result = uri.get_upstream_and_protocol().unwrap();

        assert_eq!(result.0.host_str(), "example.com");
        assert_eq!(result.0.port(), 80);
        assert_eq!(result.1, HttpProxySubProtocol::HttpForward);

        // HTTPS
        let uri = Uri::from_static("https://example.com/path");
        let result = uri.get_upstream_and_protocol().unwrap();

        assert_eq!(result.0.host_str(), "example.com");
        assert_eq!(result.0.port(), 443);
        assert_eq!(result.1, HttpProxySubProtocol::HttpsForward);

        // FTP
        let uri = Uri::from_static("ftp://example.com/path");
        let result = uri.get_upstream_and_protocol().unwrap();

        assert_eq!(result.0.host_str(), "example.com");
        assert_eq!(result.0.port(), 21);
        assert_eq!(result.1, HttpProxySubProtocol::FtpOverHttp);
    }

    #[test]
    fn get_upstream_with_default_port() {
        let uri = Uri::from_static("http://example.com:8080/path");
        let result = uri.get_upstream_with_default_port(80).unwrap();

        assert_eq!(result.host_str(), "example.com");
        assert_eq!(result.port(), 8080);

        let uri = Uri::from_static("http://example.com/path");
        let result = uri.get_upstream_with_default_port(80).unwrap();

        assert_eq!(result.host_str(), "example.com");
        assert_eq!(result.port(), 80);

        let uri = Uri::from_static("http://[2001:db8::1]:8080/path");
        let result = uri.get_upstream_with_default_port(80).unwrap();

        assert_eq!(result.host_str(), "2001:db8::1");
        assert_eq!(result.port(), 8080);

        let uri = Uri::from_static("/path");
        let result = uri.get_upstream_with_default_port(80).unwrap_err();

        assert!(matches!(
            result,
            HttpRequestParseError::InvalidRequestTarget
        ));
    }

    #[test]
    fn get_optional_upstream_with_default_port() {
        let uri = Uri::from_static("http://example.com:8080/path");
        let result = uri.get_optional_upstream_with_default_port(80).unwrap();

        assert!(result.is_some());
        let upstream = result.unwrap();
        assert_eq!(upstream.host_str(), "example.com");
        assert_eq!(upstream.port(), 8080);

        let uri = Uri::from_static("http://example.com/path");
        let result = uri.get_optional_upstream_with_default_port(80).unwrap();

        assert!(result.is_some());
        let upstream = result.unwrap();
        assert_eq!(upstream.host_str(), "example.com");
        assert_eq!(upstream.port(), 80);

        let uri = Uri::from_static("https://[2001:db8::1]:8443/path");
        let result = uri.get_optional_upstream_with_default_port(443).unwrap();

        assert!(result.is_some());
        let upstream = result.unwrap();
        assert_eq!(upstream.host_str(), "2001:db8::1");
        assert_eq!(upstream.port(), 8443);

        let uri = Uri::from_static("/path");
        let result = uri.get_optional_upstream_with_default_port(80).unwrap();

        assert!(result.is_none());
    }

    #[test]
    fn get_connect_udp_upstream() {
        // Valid
        let uri = Uri::from_static("http://example.com/.well-known/masque/udp/192.168.1.1/53");
        let result = uri.get_connect_udp_upstream().unwrap();

        assert_eq!(result.host_str(), "192.168.1.1");
        assert_eq!(result.port(), 53);

        let uri = Uri::from_static("http://example.com/.well-known/masque/udp/example.com/53");
        let result = uri.get_connect_udp_upstream().unwrap();

        assert_eq!(result.host_str(), "example.com");
        assert_eq!(result.port(), 53);

        let uri =
            Uri::from_static("http://example.com/.well-known/masque/udp/192.168.1.1/53/extra");
        let result = uri.get_connect_udp_upstream().unwrap();

        assert_eq!(result.host_str(), "192.168.1.1");
        assert_eq!(result.port(), 53);

        let uri = Uri::from_static("http://example.com/.well-known/masque/udp/2001:db8::1/53");
        let result = uri.get_connect_udp_upstream().unwrap();

        assert_eq!(result.host_str(), "2001:db8::1");
        assert_eq!(result.port(), 53);

        // Invalid
        let uri = Uri::from_static("http://example.com/other-path/udp/192.168.1.1/53");
        let result = uri.get_connect_udp_upstream().unwrap_err();

        assert!(matches!(
            result,
            HttpRequestParseError::InvalidRequestTarget
        ));

        let uri = Uri::from_static("http://example.com/.well-known/masque/udp//53");
        let result = uri.get_connect_udp_upstream().unwrap_err();

        assert!(matches!(
            result,
            HttpRequestParseError::InvalidRequestTarget
        ));

        let uri = Uri::from_static("http://example.com/.well-known/masque/udp/192.168.1.1/");
        let result = uri.get_connect_udp_upstream().unwrap_err();

        assert!(matches!(
            result,
            HttpRequestParseError::InvalidRequestTarget
        ));

        let uri = Uri::from_static("http://example.com/.well-known/masque/udp/192.168.1.1/invalid");
        let result = uri.get_connect_udp_upstream().unwrap_err();

        assert!(matches!(
            result,
            HttpRequestParseError::InvalidRequestTarget
        ));
    }
}
