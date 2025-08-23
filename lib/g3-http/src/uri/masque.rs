/*
 * SPDX-License-Identifier: Apache-2.0
 * Copyright 2025 ByteDance and/or its affiliates.
 */

use std::str::FromStr;

use http::Uri;
use percent_encoding::percent_decode_str;

use g3_types::net::{Host, UpstreamAddr};

use super::UriParseError;

#[derive(Debug)]
pub enum HttpMasque {
    Udp(UpstreamAddr),
    Ip(Option<Host>, Option<u16>),
    Http(Uri),
}

impl HttpMasque {
    pub(super) fn new_udp(host: &str, port: &str) -> Result<Self, UriParseError> {
        let host = Host::from_str(host).map_err(|_| UriParseError::NotValidHost("target_host"))?;
        let port = u16::from_str(port).map_err(|_| UriParseError::NotValidPort("target_port"))?;
        Ok(HttpMasque::Udp(UpstreamAddr::new(host, port)))
    }

    pub(super) fn new_ip(host: &str, proto: &str) -> Result<Self, UriParseError> {
        let host = if host.eq("*") {
            None
        } else {
            Some(Host::from_str(host).map_err(|_| UriParseError::NotValidHost("target"))?)
        };
        let proto = if proto.eq("*") {
            None
        } else {
            Some(u16::from_str(proto).map_err(|_| UriParseError::NotValidProtocol("ipproto"))?)
        };
        Ok(HttpMasque::Ip(host, proto))
    }

    pub(super) fn new_http(uri: &str) -> Result<Self, UriParseError> {
        let decoded = percent_decode_str(uri)
            .decode_utf8()
            .map_err(|_| UriParseError::NotValidUri("target_uri"))?;
        let uri = Uri::from_str(&decoded).map_err(|_| UriParseError::NotValidUri("target_uri"))?;
        Ok(HttpMasque::Http(uri))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    mod new_udp {
        use super::*;

        #[test]
        fn valid_host_and_port() {
            let result = HttpMasque::new_udp("example.com", "8080").unwrap();
            let HttpMasque::Udp(addr) = result else {
                panic!("not a udp masque")
            };
            assert_eq!(addr.host_str(), "example.com");
            assert_eq!(addr.port(), 8080);
        }

        #[test]
        fn invalid_host() {
            let err = HttpMasque::new_udp("::invalid::", "8080").unwrap_err();
            assert!(matches!(err, UriParseError::NotValidHost("target_host")));
        }

        #[test]
        fn invalid_port_non_number() {
            let err = HttpMasque::new_udp("example.com", "not_a_port").unwrap_err();
            assert!(matches!(err, UriParseError::NotValidPort("target_port")));
        }

        #[test]
        fn invalid_port_out_of_range() {
            let err = HttpMasque::new_udp("example.com", "65536").unwrap_err();
            assert!(matches!(err, UriParseError::NotValidPort("target_port")));
        }
    }

    mod new_ip {
        use super::*;

        #[test]
        fn wildcard_host_and_proto() {
            let result = HttpMasque::new_ip("*", "*").unwrap();
            assert!(matches!(result, HttpMasque::Ip(None, None)));
        }

        #[test]
        fn valid_host_and_proto() {
            let result = HttpMasque::new_ip("example.com", "6").unwrap();
            assert!(matches!(result, HttpMasque::Ip(Some(_), Some(6))));
        }

        #[test]
        fn invalid_host_non_wildcard() {
            let result = HttpMasque::new_ip("::invalid::", "*").unwrap_err();
            assert!(matches!(result, UriParseError::NotValidHost("target")));
        }

        #[test]
        fn invalid_proto_non_wildcard() {
            let result = HttpMasque::new_ip("*", "not_a_number").unwrap_err();
            assert!(matches!(result, UriParseError::NotValidProtocol("ipproto")));
        }
    }

    mod new_http {
        use super::*;

        #[test]
        fn valid_encoded_uri() {
            let result = HttpMasque::new_http("http%3A%2F%2Fexample.com%2Fpath").unwrap();
            let HttpMasque::Http(uri) = result else {
                panic!("not a http masque")
            };
            assert_eq!(uri.scheme_str(), Some("http"));
            assert_eq!(uri.host(), Some("example.com"));
            assert_eq!(uri.path(), "/path");
        }

        #[test]
        fn valid_unencoded_uri() {
            let result = HttpMasque::new_http("http://example.com/path").unwrap();
            assert!(matches!(result, HttpMasque::Http(_)));
        }

        #[test]
        fn invalid_encoding() {
            let result = HttpMasque::new_http("http%ZZexample.com").unwrap_err();
            assert!(matches!(result, UriParseError::NotValidUri("target_uri")));
        }

        #[test]
        fn invalid_uri_after_decoding() {
            let result = HttpMasque::new_http(":not:a:valid:uri:").unwrap_err();
            assert!(matches!(result, UriParseError::NotValidUri("target_uri")));
        }

        #[test]
        fn complex_uri_with_query() {
            let result =
                HttpMasque::new_http("https%3A%2F%2Fapi.example.com%2Fv1%2Fdata%3Fkey%3Dvalue")
                    .unwrap();
            assert!(matches!(result, HttpMasque::Http(_)));
        }
    }
}
