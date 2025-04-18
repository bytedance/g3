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

use http::Uri;
use smol_str::SmolStr;

use g3_types::net::{HttpProxySubProtocol, UpstreamAddr};

use super::{HttpMasque, UriParseError};

mod easy_proxy;
mod masque;

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
    fn easy_proxy() {
        let uri = Uri::from_static("/.well-known/easy-proxy/http/www.example.net/80/get?name=foo");
        let parsed = WellKnownUri::parse(&uri).unwrap().unwrap();
        let WellKnownUri::EasyProxy(protocol, addr, uri) = parsed else {
            panic!("not parsed as easy-proxy")
        };
        assert_eq!(protocol, HttpProxySubProtocol::HttpForward);
        assert_eq!(addr.port(), 80);
        assert_eq!(addr.host_str(), "www.example.net");
        let scheme = uri.scheme_str().unwrap();
        assert_eq!(scheme, "http");
        let authority = uri.authority().unwrap().as_str();
        assert_eq!(authority, "www.example.net:80");
        assert_eq!(uri.path(), "/get");
        let query = uri.query().unwrap();
        assert_eq!(query, "name=foo");
    }

    #[test]
    fn masque_udp() {
        let uri = Uri::from_static("/.well-known/masque/udp/192.0.2.6/443/");
        let parsed = WellKnownUri::parse(&uri).unwrap().unwrap();
        let WellKnownUri::Masque(HttpMasque::Udp(addr)) = parsed else {
            panic!("not parsed as easy-proxy")
        };
        assert_eq!(addr.port(), 443);
        assert_eq!(addr.host_str(), "192.0.2.6");
    }

    #[test]
    fn masque_ip() {
        let uri = Uri::from_static("/.well-known/masque/ip/*/*/");
        let parsed = WellKnownUri::parse(&uri).unwrap().unwrap();
        let WellKnownUri::Masque(HttpMasque::Ip(host, proto)) = parsed else {
            panic!("not parsed as easy-proxy")
        };
        assert!(host.is_none());
        assert!(proto.is_none());

        let uri = Uri::from_static("/.well-known/masque/ip/target.example.com/17/");
        let parsed = WellKnownUri::parse(&uri).unwrap().unwrap();
        let WellKnownUri::Masque(HttpMasque::Ip(host, proto)) = parsed else {
            panic!("not parsed as easy-proxy")
        };
        let host = host.unwrap();
        assert_eq!(host.to_string().as_str(), "target.example.com");
        let proto = proto.unwrap();
        assert_eq!(proto, 17);
    }
}
