/*
 * SPDX-License-Identifier: Apache-2.0
 * Copyright 2026 G3-OSS developers.
 */

use std::net::IpAddr;
use std::str::FromStr;

use anyhow::anyhow;
use ip_network::IpNetwork;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum FactsMatchType {
    ClientIp,
    ServerIp,
}

impl FromStr for FactsMatchType {
    type Err = anyhow::Error;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        match s.to_ascii_lowercase().replace('-', "_").as_str() {
            "client_ip" => Ok(FactsMatchType::ClientIp),
            "server_ip" => Ok(FactsMatchType::ServerIp),
            _ => Err(anyhow!("invalid facts match type {s}")),
        }
    }
}

#[derive(Clone, Debug)]
pub enum FactsMatchValue {
    Ip(IpAddr),
    Network(IpNetwork),
}

impl FactsMatchValue {
    pub fn new(ty: &str, value: &str) -> anyhow::Result<Self> {
        match ty.to_ascii_lowercase().as_str() {
            "ip" => {
                let v = IpAddr::from_str(value)
                    .map_err(|e| anyhow!("invalid ip address value {value}: {e}"))?;
                Ok(FactsMatchValue::Ip(v))
            }
            "net" => {
                let v = IpNetwork::from_str(value)
                    .map_err(|e| anyhow!("invalid ip network value {value}: {e}"))?;
                if (v.is_ipv4() && v.netmask() == 32) || (v.is_ipv6() && v.netmask() == 128) {
                    Ok(FactsMatchValue::Ip(v.network_address()))
                } else {
                    Ok(FactsMatchValue::Network(v))
                }
            }
            _ => Err(anyhow!("invalid facts match value type {ty}")),
        }
    }
}
