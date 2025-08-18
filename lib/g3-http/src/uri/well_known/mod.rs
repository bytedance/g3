/*
 * SPDX-License-Identifier: Apache-2.0
 * Copyright 2025 ByteDance and/or its affiliates.
 */

use http::Uri;
use smol_str::SmolStr;

use g3_types::net::{HttpProxySubProtocol, UpstreamAddr};

use super::{HttpMasque, UriParseError};

mod easy_proxy;
mod masque;

#[derive(Debug)]
pub enum WellKnownUri {
    EasyProxy(HttpProxySubProtocol, UpstreamAddr, Uri),
    Masque(HttpMasque),
    Unsupported(SmolStr),
}

struct WellKnownUriParser<'a> {
    uri: &'a Uri,
    path_offset: usize,
}

impl<'a> WellKnownUriParser<'a> {
    pub fn new(uri: &'a Uri) -> Self {
        WellKnownUriParser {
            uri,
            path_offset: 0,
        }
    }

    pub fn parse(mut self) -> Result<Option<WellKnownUri>, UriParseError> {
        let Some(magic) = self.next_path_segment() else {
            return Ok(None);
        };
        if magic != ".well-known" {
            return Ok(None);
        }

        let Some(name) = self.next_path_segment() else {
            return Ok(None);
        };
        let v = match name {
            "easy-proxy" => self.parse_easy_proxy()?,
            "masque" => self.parse_masque()?,
            _ => WellKnownUri::Unsupported(SmolStr::from(name)),
        };
        Ok(Some(v))
    }

    fn next_path_segment(&mut self) -> Option<&'a str> {
        loop {
            let left = &self.uri.path()[self.path_offset..];
            if left.is_empty() {
                return None;
            }

            match memchr::memchr(b'/', left.as_bytes()) {
                Some(0) => self.path_offset += 1,
                Some(p) => {
                    self.path_offset += p + 1;
                    return Some(&left[..p]);
                }
                None => return Some(left),
            }
        }
    }
}

impl WellKnownUri {
    pub fn parse(uri: &Uri) -> Result<Option<WellKnownUri>, UriParseError> {
        WellKnownUriParser::new(uri).parse()
    }

    pub fn suffix(&self) -> &str {
        match self {
            WellKnownUri::EasyProxy(_, _, _) => "easy-proxy",
            WellKnownUri::Masque(_) => "masque",
            WellKnownUri::Unsupported(s) => s.as_str(),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn next_path_segment_empty() {
        let uri = Uri::from_static("/");
        let mut parser = WellKnownUriParser::new(&uri);
        assert_eq!(parser.next_path_segment(), None);
    }

    #[test]
    fn next_path_segment_single() {
        let uri = Uri::from_static("/test/");
        let mut parser = WellKnownUriParser::new(&uri);
        assert_eq!(parser.next_path_segment(), Some("test"));
        assert_eq!(parser.next_path_segment(), None);
    }

    #[test]
    fn next_path_segment_multiple() {
        let uri = Uri::from_static("/a/b/c/");
        let mut parser = WellKnownUriParser::new(&uri);
        assert_eq!(parser.next_path_segment(), Some("a"));
        assert_eq!(parser.next_path_segment(), Some("b"));
        assert_eq!(parser.next_path_segment(), Some("c"));
        assert_eq!(parser.next_path_segment(), None);
    }

    #[test]
    fn next_path_segment_with_leading_slash() {
        let uri = Uri::from_static("//a/b/");
        let mut parser = WellKnownUriParser::new(&uri);
        assert_eq!(parser.next_path_segment(), Some("a"));
        assert_eq!(parser.next_path_segment(), Some("b"));
        assert_eq!(parser.next_path_segment(), None);
    }

    #[test]
    fn next_path_segment_only_slashes() {
        let uri = Uri::from_static("///");
        let mut parser = WellKnownUriParser::new(&uri);
        assert_eq!(parser.next_path_segment(), None);
    }

    #[test]
    fn parse_empty_uri() {
        let uri = Uri::default();
        assert!(WellKnownUri::parse(&uri).unwrap().is_none());
    }

    #[test]
    fn parse_non_well_known() {
        let uri = Uri::from_static("/other-path");
        assert!(WellKnownUri::parse(&uri).unwrap().is_none());
    }

    #[test]
    fn parse_missing_protocol() {
        let uri = Uri::from_static("/.well-known/");
        assert!(WellKnownUri::parse(&uri).unwrap().is_none());
    }

    #[test]
    fn parse_unsupported_protocol() {
        let uri = Uri::from_static("/.well-known/unknown");
        let result = WellKnownUri::parse(&uri).unwrap().unwrap();
        assert_eq!(result.suffix(), "unknown")
    }

    #[test]
    fn parse_easy_proxy() {
        let uri = Uri::from_static("/.well-known/easy-proxy/http/target.com/80/path?query=1");
        let result = WellKnownUri::parse(&uri).unwrap().unwrap();
        assert_eq!(result.suffix(), "easy-proxy");
    }

    #[test]
    fn parse_masque_udp() {
        let uri = Uri::from_static("/.well-known/masque/udp/192.0.2.1/53");
        let result = WellKnownUri::parse(&uri).unwrap().unwrap();
        assert_eq!(result.suffix(), "masque");
    }

    #[test]
    fn parse_masque_http() {
        let uri = Uri::from_static("/.well-known/masque/http/http%3A%2F%2Fexample.com");
        let result = WellKnownUri::parse(&uri).unwrap().unwrap();
        assert_eq!(result.suffix(), "masque");
    }

    #[test]
    fn parse_error_invalid_masque_segment() {
        let uri = Uri::from_static("/.well-known/masque/invalid");
        let result = WellKnownUri::parse(&uri).unwrap().unwrap();
        assert_eq!(result.suffix(), "masque/invalid")
    }
}
