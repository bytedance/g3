/*
 * SPDX-License-Identifier: Apache-2.0
 * Copyright 2023-2025 ByteDance and/or its affiliates.
 */

use std::net::{IpAddr, SocketAddr};
use std::str::FromStr;

use http::HeaderName;

use crate::net::{HttpHeaderMap, HttpHeaderValue};

#[derive(Clone, Copy, Debug)]
pub struct HttpStandardForwardedHeaderValue {
    for_addr: SocketAddr,
    by_addr: SocketAddr,
}

#[derive(Clone, Copy, Debug)]
pub enum HttpForwardedHeaderValue {
    Classic(IpAddr),
    Standard(HttpStandardForwardedHeaderValue),
}

impl HttpForwardedHeaderValue {
    pub fn new_classic(ip: IpAddr) -> Self {
        HttpForwardedHeaderValue::Classic(ip)
    }

    pub fn new_standard(for_addr: SocketAddr, by_addr: SocketAddr) -> Self {
        HttpForwardedHeaderValue::Standard(HttpStandardForwardedHeaderValue { for_addr, by_addr })
    }

    pub fn append_to(&self, map: &mut HttpHeaderMap) {
        match self {
            HttpForwardedHeaderValue::Classic(ip) => {
                let name = HeaderName::from_static("x-forwarded-for");
                map.append(name, unsafe {
                    HttpHeaderValue::from_string_unchecked(ip.to_string())
                });
            }
            HttpForwardedHeaderValue::Standard(HttpStandardForwardedHeaderValue {
                for_addr,
                by_addr,
            }) => {
                let s = match (for_addr, by_addr) {
                    (SocketAddr::V4(f), SocketAddr::V4(b)) => {
                        format!("for={f}; by={b}")
                    }
                    (SocketAddr::V4(f), SocketAddr::V6(b)) => {
                        format!("for={f}; by=\"{b}\"")
                    }
                    (SocketAddr::V6(f), SocketAddr::V4(b)) => {
                        format!("for=\"{f}\"; by={b}")
                    }
                    (SocketAddr::V6(f), SocketAddr::V6(b)) => {
                        format!("for=\"{f}\"; by=\"{b}\"")
                    }
                };
                map.append(http::header::FORWARDED, unsafe {
                    HttpHeaderValue::from_string_unchecked(s)
                });
            }
        }
    }

    pub fn build_header_line(&self) -> String {
        match self {
            HttpForwardedHeaderValue::Classic(ip) => {
                format!("X-Forwarded-For: {ip}\r\n")
            }
            HttpForwardedHeaderValue::Standard(HttpStandardForwardedHeaderValue {
                for_addr,
                by_addr,
            }) => match (for_addr, by_addr) {
                (SocketAddr::V4(f), SocketAddr::V4(b)) => {
                    format!("Forwarded: for={f}; by={b}\r\n")
                }
                (SocketAddr::V4(f), SocketAddr::V6(b)) => {
                    format!("Forwarded: for={f}; by=\"{b}\"\r\n")
                }
                (SocketAddr::V6(f), SocketAddr::V4(b)) => {
                    format!("Forwarded: for=\"{f}\"; by={b}\r\n")
                }
                (SocketAddr::V6(f), SocketAddr::V6(b)) => {
                    format!("Forwarded: for=\"{f}\"; by=\"{b}\"\r\n")
                }
            },
        }
    }
}

#[derive(Clone, Copy, Default, Debug, Eq, PartialEq)]
pub enum HttpForwardedHeaderType {
    #[default]
    Classic,
    Standard,
    Disable,
}

impl FromStr for HttpForwardedHeaderType {
    type Err = ();

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        match s.to_lowercase().as_str() {
            "none" | "disable" => Ok(HttpForwardedHeaderType::Disable),
            "classic" | "enable" => Ok(HttpForwardedHeaderType::Classic),
            "standard" | "rfc7239" => Ok(HttpForwardedHeaderType::Standard),
            _ => Err(()),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::net::{Ipv4Addr, Ipv6Addr};

    #[test]
    fn http_forwarded_header_value_operations() {
        // constructors
        let ipv4 = IpAddr::V4(Ipv4Addr::new(192, 168, 1, 1));
        let classic_value = HttpForwardedHeaderValue::new_classic(ipv4);
        if let HttpForwardedHeaderValue::Classic(ip) = classic_value {
            assert_eq!(ip, ipv4);
        } else {
            panic!("Expected Classic variant");
        }

        let ipv6 = IpAddr::V6(Ipv6Addr::new(0x2001, 0xdb8, 0, 0, 0, 0, 0, 1));
        let classic_value = HttpForwardedHeaderValue::new_classic(ipv6);
        if let HttpForwardedHeaderValue::Classic(ip) = classic_value {
            assert_eq!(ip, ipv6);
        } else {
            panic!("Expected Classic variant");
        }

        let for_addr_v4 = SocketAddr::new(IpAddr::V4(Ipv4Addr::new(192, 168, 1, 1)), 8080);
        let by_addr_v4 = SocketAddr::new(IpAddr::V4(Ipv4Addr::new(10, 0, 0, 1)), 9090);
        let standard_value = HttpForwardedHeaderValue::new_standard(for_addr_v4, by_addr_v4);
        if let HttpForwardedHeaderValue::Standard(HttpStandardForwardedHeaderValue {
            for_addr: f,
            by_addr: b,
        }) = standard_value
        {
            assert_eq!(f, for_addr_v4);
            assert_eq!(b, by_addr_v4);
        } else {
            panic!("Expected Standard variant");
        }

        let for_addr_v6 = SocketAddr::new(
            IpAddr::V6(Ipv6Addr::new(0x2001, 0xdb8, 0, 0, 0, 0, 0, 1)),
            8080,
        );
        let by_addr_v6 = SocketAddr::new(
            IpAddr::V6(Ipv6Addr::new(0x2001, 0xdb8, 0, 0, 0, 0, 0, 2)),
            9090,
        );
        let standard_value = HttpForwardedHeaderValue::new_standard(for_addr_v6, by_addr_v6);
        if let HttpForwardedHeaderValue::Standard(HttpStandardForwardedHeaderValue {
            for_addr: f,
            by_addr: b,
        }) = standard_value
        {
            assert_eq!(f, for_addr_v6);
            assert_eq!(b, by_addr_v6);
        } else {
            panic!("Expected Standard variant");
        }

        // append_to for all IP combinations
        let test_cases = vec![
            (
                SocketAddr::new(IpAddr::V4(Ipv4Addr::new(192, 168, 1, 1)), 8080),
                SocketAddr::new(IpAddr::V4(Ipv4Addr::new(10, 0, 0, 1)), 9090),
                "for=192.168.1.1:8080; by=10.0.0.1:9090",
            ),
            (
                SocketAddr::new(IpAddr::V4(Ipv4Addr::new(192, 168, 1, 1)), 8080),
                SocketAddr::new(
                    IpAddr::V6(Ipv6Addr::new(0x2001, 0xdb8, 0, 0, 0, 0, 0, 1)),
                    9090,
                ),
                "for=192.168.1.1:8080; by=\"[2001:db8::1]:9090\"",
            ),
            (
                SocketAddr::new(
                    IpAddr::V6(Ipv6Addr::new(0x2001, 0xdb8, 0, 0, 0, 0, 0, 1)),
                    8080,
                ),
                SocketAddr::new(IpAddr::V4(Ipv4Addr::new(10, 0, 0, 1)), 9090),
                "for=\"[2001:db8::1]:8080\"; by=10.0.0.1:9090",
            ),
            (
                SocketAddr::new(
                    IpAddr::V6(Ipv6Addr::new(0x2001, 0xdb8, 0, 0, 0, 0, 0, 1)),
                    8080,
                ),
                SocketAddr::new(
                    IpAddr::V6(Ipv6Addr::new(0x2001, 0xdb8, 0, 0, 0, 0, 0, 2)),
                    9090,
                ),
                "for=\"[2001:db8::1]:8080\"; by=\"[2001:db8::2]:9090\"",
            ),
        ];

        for (for_addr, by_addr, expected) in test_cases {
            let mut map = HttpHeaderMap::default();
            let standard_value = HttpForwardedHeaderValue::new_standard(for_addr, by_addr);
            standard_value.append_to(&mut map);

            assert!(map.contains_key("forwarded"));
            let header_value = map.get("forwarded").unwrap();
            assert_eq!(header_value.to_str(), expected);
        }

        let mut map = HttpHeaderMap::default();
        let classic_value = HttpForwardedHeaderValue::new_classic(ipv4);
        classic_value.append_to(&mut map);

        assert!(map.contains_key("x-forwarded-for"));
        let header_value = map.get("x-forwarded-for").unwrap();
        assert_eq!(header_value.to_str(), "192.168.1.1");

        // build_header_line for all IP combinations
        let header_line = classic_value.build_header_line();
        assert_eq!(header_line, "X-Forwarded-For: 192.168.1.1\r\n");

        let test_cases = vec![
            (
                SocketAddr::new(IpAddr::V4(Ipv4Addr::new(192, 168, 1, 1)), 8080),
                SocketAddr::new(IpAddr::V4(Ipv4Addr::new(10, 0, 0, 1)), 9090),
                "Forwarded: for=192.168.1.1:8080; by=10.0.0.1:9090\r\n",
            ),
            (
                SocketAddr::new(IpAddr::V4(Ipv4Addr::new(192, 168, 1, 1)), 8080),
                SocketAddr::new(
                    IpAddr::V6(Ipv6Addr::new(0x2001, 0xdb8, 0, 0, 0, 0, 0, 1)),
                    9090,
                ),
                "Forwarded: for=192.168.1.1:8080; by=\"[2001:db8::1]:9090\"\r\n",
            ),
            (
                SocketAddr::new(
                    IpAddr::V6(Ipv6Addr::new(0x2001, 0xdb8, 0, 0, 0, 0, 0, 1)),
                    8080,
                ),
                SocketAddr::new(IpAddr::V4(Ipv4Addr::new(10, 0, 0, 1)), 9090),
                "Forwarded: for=\"[2001:db8::1]:8080\"; by=10.0.0.1:9090\r\n",
            ),
            (
                SocketAddr::new(
                    IpAddr::V6(Ipv6Addr::new(0x2001, 0xdb8, 0, 0, 0, 0, 0, 1)),
                    8080,
                ),
                SocketAddr::new(
                    IpAddr::V6(Ipv6Addr::new(0x2001, 0xdb8, 0, 0, 0, 0, 0, 2)),
                    9090,
                ),
                "Forwarded: for=\"[2001:db8::1]:8080\"; by=\"[2001:db8::2]:9090\"\r\n",
            ),
        ];

        for (for_addr, by_addr, expected) in test_cases {
            let standard_value = HttpForwardedHeaderValue::new_standard(for_addr, by_addr);
            let header_line = standard_value.build_header_line();
            assert_eq!(header_line, expected);
        }
    }

    #[test]
    fn http_forwarded_header_type_operations() {
        // valid cases
        assert_eq!(
            "none".parse::<HttpForwardedHeaderType>().unwrap(),
            HttpForwardedHeaderType::Disable
        );
        assert_eq!(
            "disable".parse::<HttpForwardedHeaderType>().unwrap(),
            HttpForwardedHeaderType::Disable
        );
        assert_eq!(
            "classic".parse::<HttpForwardedHeaderType>().unwrap(),
            HttpForwardedHeaderType::Classic
        );
        assert_eq!(
            "enable".parse::<HttpForwardedHeaderType>().unwrap(),
            HttpForwardedHeaderType::Classic
        );
        assert_eq!(
            "standard".parse::<HttpForwardedHeaderType>().unwrap(),
            HttpForwardedHeaderType::Standard
        );
        assert_eq!(
            "rfc7239".parse::<HttpForwardedHeaderType>().unwrap(),
            HttpForwardedHeaderType::Standard
        );

        // case insensitivity
        assert_eq!(
            "NONE".parse::<HttpForwardedHeaderType>().unwrap(),
            HttpForwardedHeaderType::Disable
        );
        assert_eq!(
            "DISABLE".parse::<HttpForwardedHeaderType>().unwrap(),
            HttpForwardedHeaderType::Disable
        );
        assert_eq!(
            "CLASSIC".parse::<HttpForwardedHeaderType>().unwrap(),
            HttpForwardedHeaderType::Classic
        );
        assert_eq!(
            "ENABLE".parse::<HttpForwardedHeaderType>().unwrap(),
            HttpForwardedHeaderType::Classic
        );
        assert_eq!(
            "STANDARD".parse::<HttpForwardedHeaderType>().unwrap(),
            HttpForwardedHeaderType::Standard
        );
        assert_eq!(
            "RFC7239".parse::<HttpForwardedHeaderType>().unwrap(),
            HttpForwardedHeaderType::Standard
        );

        // invalid cases
        assert!("invalid".parse::<HttpForwardedHeaderType>().is_err());
        assert!("".parse::<HttpForwardedHeaderType>().is_err());
        assert!("unknown".parse::<HttpForwardedHeaderType>().is_err());

        // default value
        assert_eq!(
            HttpForwardedHeaderType::default(),
            HttpForwardedHeaderType::Classic
        );
    }
}
