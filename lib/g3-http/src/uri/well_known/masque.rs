/*
 * SPDX-License-Identifier: Apache-2.0
 * Copyright 2025 ByteDance and/or its affiliates.
 */

use smol_str::SmolStr;

use super::{WellKnownUri, WellKnownUriParser};
use crate::uri::{HttpMasque, UriParseError};

impl WellKnownUriParser<'_> {
    pub(super) fn parse_masque(&mut self) -> Result<WellKnownUri, UriParseError> {
        let Some(segment) = self.next_path_segment() else {
            return Err(UriParseError::RequiredFieldNotFound("segment"));
        };
        match segment {
            "udp" => {
                let Some(host) = self.next_path_segment() else {
                    return Err(UriParseError::RequiredFieldNotFound("target_host"));
                };

                let Some(port) = self.next_path_segment() else {
                    return Err(UriParseError::RequiredFieldNotFound("target_port"));
                };

                let masque = HttpMasque::new_udp(host, port)?;
                Ok(WellKnownUri::Masque(masque))
            }
            "ip" => {
                let Some(host) = self.next_path_segment() else {
                    return Err(UriParseError::RequiredFieldNotFound("target"));
                };

                let Some(proto) = self.next_path_segment() else {
                    return Err(UriParseError::RequiredFieldNotFound("ipproto"));
                };

                let masque = HttpMasque::new_ip(host, proto)?;
                Ok(WellKnownUri::Masque(masque))
            }
            "http" => {
                let Some(uri) = self.next_path_segment() else {
                    return Err(UriParseError::RequiredFieldNotFound("target_uri"));
                };

                let masque = HttpMasque::new_http(uri)?;
                Ok(WellKnownUri::Masque(masque))
            }
            _ => Ok(WellKnownUri::Unsupported(SmolStr::from_iter([
                "masque", "/", segment,
            ]))),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use http::Uri;

    fn setup_parser(uri_str: &'static str) -> Result<WellKnownUri, UriParseError> {
        let uri = Uri::from_static(uri_str);
        let mut parser = WellKnownUriParser::new(&uri);
        parser.next_path_segment();
        parser.next_path_segment();
        parser.parse_masque()
    }

    #[test]
    fn missing_segment() {
        let err = setup_parser("/.well-known/masque/").unwrap_err();
        assert!(matches!(
            err,
            UriParseError::RequiredFieldNotFound("segment")
        ));
    }

    #[test]
    fn udp_missing_host() {
        let err = setup_parser("/.well-known/masque/udp/").unwrap_err();
        assert!(matches!(
            err,
            UriParseError::RequiredFieldNotFound("target_host")
        ));
    }

    #[test]
    fn udp_invalid_host() {
        let err = setup_parser("/.well-known/masque/udp/::invalid::/443/").unwrap_err();
        assert!(matches!(err, UriParseError::NotValidHost("target_host")));
    }

    #[test]
    fn udp_missing_port() {
        let err = setup_parser("/.well-known/masque/udp/example.com/").unwrap_err();
        assert!(matches!(
            err,
            UriParseError::RequiredFieldNotFound("target_port")
        ));
    }

    #[test]
    fn udp_invalid_port() {
        let err = setup_parser("/.well-known/masque/udp/example.com/not_number/").unwrap_err();
        assert!(matches!(err, UriParseError::NotValidPort("target_port")));
    }

    #[test]
    fn udp_valid() {
        let parsed = setup_parser("/.well-known/masque/udp/192.0.2.6/443/").unwrap();
        let WellKnownUri::Masque(HttpMasque::Udp(addr)) = parsed else {
            panic!("not parsed as masque/udp")
        };
        assert_eq!(addr.host_str(), "192.0.2.6");
        assert_eq!(addr.port(), 443);
    }

    #[test]
    fn ip_missing_target() {
        let err = setup_parser("/.well-known/masque/ip/").unwrap_err();
        assert!(matches!(
            err,
            UriParseError::RequiredFieldNotFound("target")
        ));
    }

    #[test]
    fn ip_invalid_target() {
        let err = setup_parser("/.well-known/masque/ip/::invalid::/17/").unwrap_err();
        assert!(matches!(err, UriParseError::NotValidHost("target")));
    }

    #[test]
    fn ip_missing_proto() {
        let err = setup_parser("/.well-known/masque/ip/example.com/").unwrap_err();
        assert!(matches!(
            err,
            UriParseError::RequiredFieldNotFound("ipproto")
        ));
    }

    #[test]
    fn ip_invalid_proto() {
        let err = setup_parser("/.well-known/masque/ip/example.com/not_number/").unwrap_err();
        assert!(matches!(err, UriParseError::NotValidProtocol("ipproto")));
    }

    #[test]
    fn ip_valid() {
        let parsed = setup_parser("/.well-known/masque/ip/target.example.com/17/").unwrap();
        let WellKnownUri::Masque(HttpMasque::Ip(host, proto)) = parsed else {
            panic!("not parsed as masque/ip")
        };
        assert_eq!(host.unwrap().to_string(), "target.example.com");
        assert_eq!(proto.unwrap(), 17);
    }

    #[test]
    fn ip_wildcards() {
        let parsed = setup_parser("/.well-known/masque/ip/*/*/").unwrap();
        let WellKnownUri::Masque(HttpMasque::Ip(host, proto)) = parsed else {
            panic!("not parsed as masque/ip")
        };
        assert!(host.is_none());
        assert!(proto.is_none());
    }

    #[test]
    fn http_missing_uri() {
        let err = setup_parser("/.well-known/masque/http/").unwrap_err();
        assert!(matches!(
            err,
            UriParseError::RequiredFieldNotFound("target_uri")
        ));
    }

    #[test]
    fn http_invalid_uri() {
        let err = setup_parser("/.well-known/masque/http/::invalid::/").unwrap_err();
        assert!(matches!(err, UriParseError::NotValidUri("target_uri")));
    }

    #[test]
    fn http_invalid_encoding() {
        let err = setup_parser("/.well-known/masque/http/%/").unwrap_err();
        assert!(matches!(err, UriParseError::NotValidUri("target_uri")));
    }

    #[test]
    fn http_valid() {
        let parsed =
            setup_parser("/.well-known/masque/http/http%3A%2F%2Fhttpbin.org%2Fget").unwrap();
        let WellKnownUri::Masque(HttpMasque::Http(uri)) = parsed else {
            panic!("not parsed as masque/http")
        };
        assert_eq!(uri.scheme_str(), Some("http"));
        assert_eq!(uri.host(), Some("httpbin.org"));
        assert_eq!(uri.path(), "/get");
    }

    #[test]
    fn unsupported_protocol() {
        let parsed = setup_parser("/.well-known/masque/unknown_protocol/").unwrap();
        let WellKnownUri::Unsupported(s) = parsed else {
            panic!("not parsed as unsupported")
        };
        assert_eq!(s, "masque/unknown_protocol");
    }
}
