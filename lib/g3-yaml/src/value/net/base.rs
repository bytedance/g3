/*
 * Copyright 2023 ByteDance and/or its affiliates.
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

use std::net::{IpAddr, Ipv4Addr, Ipv6Addr, SocketAddr};
use std::str::FromStr;

use anyhow::{anyhow, Context};
use url::Url;
use yaml_rust::Yaml;

#[cfg(feature = "acl-rule")]
use ip_network::IpNetwork;

use g3_types::collection::WeightedValue;
use g3_types::net::{Host, UpstreamAddr, WeightedUpstreamAddr};

pub fn as_sockaddr(value: &Yaml) -> anyhow::Result<SocketAddr> {
    if let Yaml::String(s) = value {
        let addr = SocketAddr::from_str(s).map_err(|e| anyhow!("invalid socket address: {e}"))?;
        Ok(addr)
    } else {
        Err(anyhow!(
            "yaml value type for 'SocketAddr' should be 'string'"
        ))
    }
}

pub fn as_weighted_sockaddr(value: &Yaml) -> anyhow::Result<WeightedValue<SocketAddr>> {
    const KEY_ADDR: &str = "addr";
    const KEY_WEIGHT: &str = "weight";

    match value {
        Yaml::Hash(map) => {
            let v = crate::hash::get_required(map, KEY_ADDR)?;
            let addr = as_sockaddr(v)
                .context(format!("invalid sockaddr string value for key {KEY_ADDR}"))?;

            if let Ok(v) = crate::hash::get_required(map, KEY_WEIGHT) {
                let weight = crate::value::as_f64(v)
                    .context(format!("invalid f64 value for key {KEY_WEIGHT}"))?;
                Ok(WeightedValue::<SocketAddr>::with_weight(addr, weight))
            } else {
                Ok(WeightedValue::new(addr))
            }
        }
        _ => {
            let s = as_sockaddr(value).context("invalid sockaddr string value")?;
            Ok(WeightedValue::new(s))
        }
    }
}

pub fn as_ipaddr(value: &Yaml) -> anyhow::Result<IpAddr> {
    if let Yaml::String(s) = value {
        let ip = IpAddr::from_str(s).map_err(|e| anyhow!("invalid ip address: {e}"))?;
        Ok(ip)
    } else {
        Err(anyhow!("yaml value type for 'IpAddr' should be 'string'"))
    }
}

pub fn as_ipv4addr(value: &Yaml) -> anyhow::Result<Ipv4Addr> {
    if let Yaml::String(s) = value {
        let ip4 = Ipv4Addr::from_str(s).map_err(|e| anyhow!("invalid ipv4 address: {e}"))?;
        Ok(ip4)
    } else {
        Err(anyhow!("yaml value type for 'Ipv4Addr' should be 'string'"))
    }
}

pub fn as_ipv6addr(value: &Yaml) -> anyhow::Result<Ipv6Addr> {
    if let Yaml::String(s) = value {
        let ip6 = Ipv6Addr::from_str(s).map_err(|e| anyhow!("invalid ipv6 address: {e}"))?;
        Ok(ip6)
    } else {
        Err(anyhow!("yaml value type for 'Ipv6Addr' should be 'string'"))
    }
}

#[cfg(feature = "acl-rule")]
pub fn as_ip_network(value: &Yaml) -> anyhow::Result<IpNetwork> {
    if let Yaml::String(s) = value {
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

pub fn as_host(value: &Yaml) -> anyhow::Result<Host> {
    if let Yaml::String(s) = value {
        if let Ok(ip) = IpAddr::from_str(s) {
            Ok(Host::Ip(ip))
        } else {
            // allow more than domain_to_ascii_strict chars
            let domain = idna::domain_to_ascii(s).map_err(|e| anyhow!("invalid host: {e}"))?;
            Ok(Host::Domain(domain))
        }
    } else {
        Err(anyhow!("yaml value type for 'Host' should be 'string'"))
    }
}

pub fn as_domain(value: &Yaml) -> anyhow::Result<String> {
    if let Yaml::String(s) = value {
        // allow more than domain_to_ascii_strict chars
        let domain = idna::domain_to_ascii(s).map_err(|e| anyhow!("invalid domain: {e}"))?;
        Ok(domain)
    } else {
        Err(anyhow!("yaml value type for 'Domain' should be 'string'"))
    }
}

pub fn as_url(value: &Yaml) -> anyhow::Result<Url> {
    if let Yaml::String(s) = value {
        let url = Url::from_str(s).map_err(|e| anyhow!("invalid url: {e}"))?;
        Ok(url)
    } else {
        Err(anyhow!("yaml value type for 'Url' should be 'string'"))
    }
}

pub fn as_upstream_addr(value: &Yaml, default_port: u16) -> anyhow::Result<UpstreamAddr> {
    if let Yaml::String(s) = value {
        let mut addr = UpstreamAddr::from_str(s).context("invalid upstream addr string")?;
        if addr.port() == 0 {
            if default_port == 0 {
                return Err(anyhow!("port is required"));
            } else {
                addr.set_port(default_port);
            }
        }
        Ok(addr)
    } else {
        Err(anyhow!(
            "yaml value type for upstream addr should be 'string'"
        ))
    }
}

pub fn as_weighted_upstream_addr(
    value: &Yaml,
    default_port: u16,
) -> anyhow::Result<WeightedUpstreamAddr> {
    match value {
        Yaml::String(_) => {
            let addr =
                as_upstream_addr(value, default_port).context("invalid upstream addr string")?;
            Ok(WeightedUpstreamAddr::new(addr))
        }
        Yaml::Hash(map) => {
            let mut addr = UpstreamAddr::empty();
            let mut weight = WeightedUpstreamAddr::DEFAULT_WEIGHT;
            crate::foreach_kv(map, |k, v| match crate::key::normalize(k).as_str() {
                "addr" | "address" => {
                    addr = as_upstream_addr(v, default_port)
                        .context(format!("invalid upstream addr value for key {k}"))?;
                    Ok(())
                }
                "weight" => {
                    weight = crate::value::as_f64(v).context("invalid weight")?;
                    Ok(())
                }
                _ => Err(anyhow!("invalid key {k}")),
            })?;

            if addr.is_empty() {
                Err(anyhow!("no valid upstream addr set"))
            } else {
                Ok(WeightedUpstreamAddr::with_weight(addr, weight))
            }
        }
        _ => Err(anyhow!("invalid 'weighted upstream addr' yaml value")),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::net::Ipv4Addr;

    #[test]
    fn as_sockaddr_correct_ipv4() {
        let addr_str = "192.168.255.250:80";
        let value = Yaml::String(String::from(addr_str));
        let addr = as_sockaddr(&value).unwrap();
        assert_eq!(
            addr,
            SocketAddr::new(IpAddr::V4(Ipv4Addr::new(192, 168, 255, 250)), 80)
        );
    }

    #[test]
    fn as_sockaddr_invalid_ipv4() {
        let addr_str = "192.168.255.250.3:80";
        let value = Yaml::String(String::from(addr_str));
        assert!(as_sockaddr(&value).is_err());
    }

    #[test]
    fn as_host_correct_ipv4() {
        let addr_str = "192.168.255.250";
        let value = Yaml::String(String::from(addr_str));
        let host = as_host(&value).unwrap();
        assert_eq!(
            host,
            Host::Ip(IpAddr::V4(Ipv4Addr::new(192, 168, 255, 250)))
        );
    }

    #[test]
    fn as_domain_idna() {
        let domain_str = "ドメイン.テスト";
        let value = Yaml::String(domain_str.to_string());
        let domain = as_domain(&value).unwrap();
        assert_eq!(domain, "xn--eckwd4c7c.xn--zckzah");
    }
}
