/*
 * SPDX-License-Identifier: Apache-2.0
 * Copyright 2024-2025 ByteDance and/or its affiliates.
 */

use g3_types::net::Host;

use super::CommandLineError;

pub(super) fn parse_host(msg: &[u8]) -> Result<Host, CommandLineError> {
    let host_b = match memchr::memchr(b' ', msg) {
        Some(p) => &msg[..p],
        None => msg,
    };
    Host::parse_smtp_host_address(host_b).ok_or(CommandLineError::InvalidClientHost)
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::net::{IpAddr, Ipv4Addr, Ipv6Addr};

    #[test]
    fn parse_host_from_bytes() {
        let result = parse_host(b"example.com").unwrap();
        assert_eq!(result, Host::Domain("example.com".into()));

        let result = parse_host(b"[192.168.1.1]").unwrap();
        assert_eq!(result, Host::Ip(IpAddr::V4(Ipv4Addr::new(192, 168, 1, 1))));

        let result = parse_host(b"Ipv6:2001:db8::1").unwrap();
        assert_eq!(
            result,
            Host::Ip(IpAddr::V6(Ipv6Addr::new(0x2001, 0xdb8, 0, 0, 0, 0, 0, 1)))
        );

        let result = parse_host(b"example.com additional content").unwrap();
        assert_eq!(result, Host::Domain("example.com".into()));

        let result = parse_host(b"[192.168.1.1] extra data").unwrap();
        assert_eq!(result, Host::Ip(IpAddr::V4(Ipv4Addr::new(192, 168, 1, 1))));

        let result = parse_host(b"Ipv6:::1 more data").unwrap();
        assert_eq!(
            result,
            Host::Ip(IpAddr::V6(Ipv6Addr::new(0, 0, 0, 0, 0, 0, 0, 1)))
        );

        let result = parse_host(b" ").unwrap_err();
        assert!(matches!(result, CommandLineError::InvalidClientHost));

        let result = parse_host(b"[256.256.256.256]").unwrap_err();
        assert!(matches!(result, CommandLineError::InvalidClientHost));

        let result = parse_host(b"[192.168.1]").unwrap_err();
        assert!(matches!(result, CommandLineError::InvalidClientHost));

        let result = parse_host(b"IPv6:2001:db8::1").unwrap_err();
        assert!(matches!(result, CommandLineError::InvalidClientHost));
    }
}
