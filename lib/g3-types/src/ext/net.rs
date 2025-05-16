/*
 * SPDX-License-Identifier: Apache-2.0
 * Copyright 2025 ByteDance and/or its affiliates.
 */

use std::net::SocketAddr;

pub trait SocketAddrExt: Sized {
    /// Converts this address to an `SocketAddr::V4` if it is an IPv4-mapped address,
    /// otherwise returns self wrapped in an `SocketAddr::V6`.
    fn to_canonical(&self) -> Self;
}

impl SocketAddrExt for SocketAddr {
    #[inline]
    fn to_canonical(&self) -> Self {
        match self {
            SocketAddr::V4(_) => *self,
            SocketAddr::V6(a6) => SocketAddr::new(a6.ip().to_canonical(), a6.port()),
        }
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
        assert_eq!(addr1.to_canonical(), addr2);

        let addr1 = SocketAddr::from_str("[fe80::d118:f3a9:deeb:c033]:80").unwrap();
        assert_eq!(addr1.to_canonical(), addr1);

        let addr1 = SocketAddr::from_str("192.168.0.1:80").unwrap();
        assert_eq!(addr1.to_canonical(), addr1);
    }
}
