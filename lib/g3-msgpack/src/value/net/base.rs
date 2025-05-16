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
