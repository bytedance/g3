/*
 * SPDX-License-Identifier: Apache-2.0
 * Copyright 2023-2025 ByteDance and/or its affiliates.
 */

use std::fmt;
use std::net::{IpAddr, SocketAddr};

use socket2::Domain;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum AddressFamily {
    Ipv4,
    Ipv6,
}

impl fmt::Display for AddressFamily {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            AddressFamily::Ipv4 => f.write_str("Ipv4"),
            AddressFamily::Ipv6 => f.write_str("Ipv6"),
        }
    }
}

impl From<AddressFamily> for Domain {
    fn from(v: AddressFamily) -> Self {
        match v {
            AddressFamily::Ipv4 => Domain::IPV4,
            AddressFamily::Ipv6 => Domain::IPV6,
        }
    }
}

impl From<&IpAddr> for AddressFamily {
    fn from(ip: &IpAddr) -> Self {
        match ip {
            IpAddr::V4(_) => AddressFamily::Ipv4,
            IpAddr::V6(_) => AddressFamily::Ipv6,
        }
    }
}

impl From<&SocketAddr> for AddressFamily {
    fn from(addr: &SocketAddr) -> Self {
        match addr {
            SocketAddr::V4(_) => AddressFamily::Ipv4,
            SocketAddr::V6(_) => AddressFamily::Ipv6,
        }
    }
}

pub fn native_socket_addr(orig: SocketAddr) -> SocketAddr {
    if let SocketAddr::V6(a6) = orig {
        // convert back ipv4 mapped address to ipv4
        SocketAddr::new(a6.ip().to_canonical(), a6.port())
    } else {
        orig
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::str::FromStr;

    #[test]
    fn convert_socket_addr() {
        let addr1 = SocketAddr::from_str("[::ffff:192.168.0.1]:80").unwrap();
        let addr2 = SocketAddr::from_str("192.168.0.1:80").unwrap();
        assert_eq!(native_socket_addr(addr1), addr2);

        let addr1 = SocketAddr::from_str("[fe80::d118:f3a9:deeb:c033]:80").unwrap();
        assert_eq!(native_socket_addr(addr1), addr1);

        let addr1 = SocketAddr::from_str("192.168.0.1:80").unwrap();
        assert_eq!(native_socket_addr(addr1), addr1);
    }
}
