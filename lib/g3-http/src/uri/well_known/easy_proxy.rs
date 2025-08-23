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

#[cfg(test)]
mod tests {
    use super::*;

    fn setup_parser(uri_str: &'static str) -> Result<WellKnownUri, UriParseError> {
        let uri = Uri::from_static(uri_str);
        let mut parser = WellKnownUriParser::new(&uri);
        parser.next_path_segment();
        parser.next_path_segment();
        parser.parse_easy_proxy()
    }

    #[test]
    fn missing_scheme() {
        let err = setup_parser("/.well-known/easy-proxy/").unwrap_err();
        assert!(matches!(
            err,
            UriParseError::RequiredFieldNotFound("scheme")
        ));
    }

    #[test]
    fn invalid_scheme() {
        let err = setup_parser("/.well-known/easy-proxy/invalid_scheme/host/80/path").unwrap_err();
        assert!(matches!(err, UriParseError::NotValidScheme("scheme")));
    }

    #[test]
    fn missing_host() {
        let err = setup_parser("/.well-known/easy-proxy/http/").unwrap_err();
        assert!(matches!(
            err,
            UriParseError::RequiredFieldNotFound("target_host")
        ));
    }

    #[test]
    fn missing_port() {
        let err = setup_parser("/.well-known/easy-proxy/http/example.com/").unwrap_err();
        assert!(matches!(
            err,
            UriParseError::RequiredFieldNotFound("target_port")
        ));
    }

    #[test]
    fn invalid_port() {
        let err =
            setup_parser("/.well-known/easy-proxy/http/example.com/not_number/path").unwrap_err();
        assert!(matches!(err, UriParseError::NotValidPort("target_port")));
    }

    #[test]
    fn valid_http() {
        let parsed =
            setup_parser("/.well-known/easy-proxy/http/www.example.com/80/get?name=foo").unwrap();
        let WellKnownUri::EasyProxy(protocol, target, uri) = parsed else {
            panic!("not parsed as easy-proxy")
        };
        assert_eq!(protocol, HttpProxySubProtocol::HttpForward);
        assert_eq!(target.host_str(), "www.example.com");
        assert_eq!(target.port(), 80);
        assert_eq!(uri.scheme_str().unwrap(), "http");
        assert_eq!(uri.authority().unwrap().as_str(), "www.example.com:80");
        assert_eq!(uri.path(), "/get");
        assert_eq!(uri.query().unwrap(), "name=foo");
    }

    #[test]
    fn valid_https() {
        let parsed =
            setup_parser("/.well-known/easy-proxy/https/secure.site/443/dashboard").unwrap();
        let WellKnownUri::EasyProxy(protocol, target, uri) = parsed else {
            panic!("not parsed as easy-proxy")
        };
        assert_eq!(protocol, HttpProxySubProtocol::HttpsForward);
        assert_eq!(target.host_str(), "secure.site");
        assert_eq!(target.port(), 443);
        assert_eq!(uri.scheme_str().unwrap(), "https");
        assert_eq!(uri.authority().unwrap().as_str(), "secure.site:443");
        assert_eq!(uri.path(), "/dashboard");
    }

    #[test]
    fn valid_ftp() {
        let parsed = setup_parser("/.well-known/easy-proxy/ftp/fileserver/21/download").unwrap();
        let WellKnownUri::EasyProxy(protocol, target, uri) = parsed else {
            panic!("not parsed as easy-proxy")
        };
        assert_eq!(protocol, HttpProxySubProtocol::FtpOverHttp);
        assert_eq!(target.host_str(), "fileserver");
        assert_eq!(target.port(), 21);
        assert_eq!(uri.scheme_str().unwrap(), "ftp");
        assert_eq!(uri.authority().unwrap().as_str(), "fileserver:21");
        assert_eq!(uri.path(), "/download");
    }

    #[test]
    fn idn_domain() {
        let parsed = setup_parser("/.well-known/easy-proxy/http/www.例子.com/80/path").unwrap();
        let WellKnownUri::EasyProxy(_, target, _) = parsed else {
            panic!("not parsed as easy-proxy")
        };
        assert_eq!(target.host_str(), "www.xn--fsqu00a.com");
    }
}
