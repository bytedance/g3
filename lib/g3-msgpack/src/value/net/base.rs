/*
 * SPDX-License-Identifier: Apache-2.0
 * Copyright 2024-2025 ByteDance and/or its affiliates.
 */

use std::net::IpAddr;
use std::str::FromStr;

use anyhow::anyhow;
#[cfg(feature = "geoip")]
use ip_network::IpNetwork;
use rmpv::ValueRef;

pub fn as_ipaddr(value: &ValueRef) -> anyhow::Result<IpAddr> {
    match value {
        ValueRef::String(s) => {
            let s = s
                .as_str()
                .ok_or(anyhow!("invalid utf-8 ip address string value"))?;
            let ip = IpAddr::from_str(s).map_err(|e| anyhow!("invalid ip address: {e}"))?;
            Ok(ip)
        }
        _ => Err(anyhow!(
            "msgpack value type for 'IpAddr' should be 'string'"
        )),
    }
}

#[cfg(feature = "geoip")]
pub fn as_ip_network(value: &ValueRef) -> anyhow::Result<IpNetwork> {
    if let ValueRef::String(s) = value {
        let s = s
            .as_str()
            .ok_or(anyhow!("invalid utf-8 ip network string value"))?;
        let net = match IpNetwork::from_str(s) {
            Ok(net) => net,
            Err(_) => match IpAddr::from_str(s) {
                Ok(IpAddr::V4(ip4)) => IpNetwork::new(ip4, 32)
                    .map_err(|_| anyhow!("failed to add ipv4 address: internal error"))?,
                Ok(IpAddr::V6(ip6)) => IpNetwork::new(ip6, 128)
                    .map_err(|_| anyhow!("failed to add ipv6 address: internal error"))?,
                Err(_) => {
                    return Err(anyhow!("invalid network or ip string: {s}"));
                }
            },
        };
        Ok(net)
    } else {
        Err(anyhow!(
            "yaml value type for 'IpNetwork' should be 'string'"
        ))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use rmpv::Integer;

    #[test]
    fn as_ipaddr_ok() {
        // valid IPv4 and IPv6 address strings
        let v = ValueRef::String("192.0.2.1".into());
        assert_eq!(
            as_ipaddr(&v).unwrap(),
            IpAddr::from_str("192.0.2.1").unwrap()
        );

        let v = ValueRef::String("2001:db8::1".into());
        assert_eq!(
            as_ipaddr(&v).unwrap(),
            IpAddr::from_str("2001:db8::1").unwrap()
        );
    }

    #[test]
    fn as_ipaddr_err() {
        // invalid IP format
        let v = ValueRef::String("invalid_ip".into());
        assert!(as_ipaddr(&v).is_err());

        // non-string type
        let v = ValueRef::Integer(Integer::from(42));
        assert!(as_ipaddr(&v).is_err());

        // invalid UTF-8
        let v = ValueRef::Binary(b"\x80");
        assert!(as_ipaddr(&v).is_err());
    }

    #[test]
    #[cfg(feature = "geoip")]
    fn as_ip_network_ok() {
        // valid network strings
        let v = ValueRef::String("192.0.2.0/24".into());
        assert_eq!(as_ip_network(&v).unwrap().to_string(), "192.0.2.0/24");

        let v = ValueRef::String("192.0.2.1".into());
        assert_eq!(as_ip_network(&v).unwrap().to_string(), "192.0.2.1/32");

        let v = ValueRef::String("2001:db8::1".into());
        assert_eq!(as_ip_network(&v).unwrap().to_string(), "2001:db8::1/128");
    }

    #[test]
    #[cfg(feature = "geoip")]
    fn as_ip_network_err() {
        // invalid network format
        let v = ValueRef::String("invalid_network".into());
        assert!(as_ip_network(&v).is_err());

        // non-string type
        let v = ValueRef::Integer(Integer::from(42));
        assert!(as_ip_network(&v).is_err());

        // invalid UTF-8
        let v = ValueRef::Binary(b"\x80");
        assert!(as_ip_network(&v).is_err());
    }
}
