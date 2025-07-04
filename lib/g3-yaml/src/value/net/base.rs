/*
 * SPDX-License-Identifier: Apache-2.0
 * Copyright 2023-2025 ByteDance and/or its affiliates.
 */

use std::net::{IpAddr, Ipv4Addr, Ipv6Addr, SocketAddr, ToSocketAddrs};
use std::str::FromStr;

use anyhow::{Context, anyhow};
use url::Url;
use yaml_rust::Yaml;

#[cfg(feature = "acl-rule")]
use ip_network::IpNetwork;

use g3_types::collection::WeightedValue;
use g3_types::net::{Host, UpstreamAddr, WeightedUpstreamAddr};

pub fn as_env_sockaddr(value: &Yaml) -> anyhow::Result<SocketAddr> {
    if let Yaml::String(s) = value {
        if let Some(var) = s.strip_prefix('$') {
            let s = std::env::var(var)
                .map_err(|e| anyhow!("failed to get environment var {var}: {e}"))?;
            SocketAddr::from_str(&s).map_err(|e| {
                anyhow!("invalid socket address {s} set in environment var {var}: {e}")
            })
        } else if let Some(addr) = s.strip_prefix('@') {
            let addrs: Vec<SocketAddr> = addr
                .to_socket_addrs()
                .map_err(|e| anyhow!("failed to resolve socket address string {addr}: {e}"))?
                .collect();
            if addrs.len() > 1 {
                return Err(anyhow!("{addr} resolved to too many addresses({addrs:?})"));
            }
            addrs
                .into_iter()
                .next()
                .ok_or_else(|| anyhow!("can not resolve {addr}"))
        } else {
            SocketAddr::from_str(s).map_err(|e| anyhow!("invalid socket address: {e}"))
        }
    } else {
        Err(anyhow!(
            "yaml value type for 'SocketAddr' should be 'string'"
        ))
    }
}

pub fn as_sockaddr(value: &Yaml) -> anyhow::Result<SocketAddr> {
    if let Yaml::String(s) = value {
        SocketAddr::from_str(s).map_err(|e| anyhow!("invalid socket address: {e}"))
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
            Ok(Host::Domain(domain.into()))
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
    use std::sync::{Mutex, OnceLock};

    // ensure the environment variable is thread-safe
    fn env_lock() -> &'static Mutex<()> {
        static ENV_LOCK: OnceLock<Mutex<()>> = OnceLock::new();
        ENV_LOCK.get_or_init(|| Mutex::new(()))
    }

    #[test]
    fn as_env_sockaddr_ok() {
        let yaml = yaml_str!("127.0.0.1:8080");
        let addr = as_env_sockaddr(&yaml).unwrap();
        assert_eq!(addr, "127.0.0.1:8080".parse().unwrap());

        {
            let _guard = env_lock().lock().unwrap(); // Acquire lock
            unsafe {
                std::env::set_var("TEST_ADDR", "192.168.1.1:9090");
            }
            let yaml = yaml_str!("$TEST_ADDR");
            let addr = as_env_sockaddr(&yaml).unwrap();
            assert_eq!(addr, "192.168.1.1:9090".parse().unwrap());
            unsafe {
                std::env::remove_var("TEST_ADDR");
            }
        } // Lock is released

        let yaml = yaml_str!("@127.0.0.1:80");
        let addr = as_env_sockaddr(&yaml).unwrap();
        assert_eq!(addr, "127.0.0.1:80".parse().unwrap());
    }

    #[test]
    fn as_env_sockaddr_err() {
        {
            let _guard = env_lock().lock().unwrap(); // Acquire lock
            unsafe {
                std::env::set_var("TEST_ADDR", "invalid_address");
            }
            let yaml = yaml_str!("$TEST_ADDR");
            assert!(as_env_sockaddr(&yaml).is_err());
            unsafe {
                std::env::remove_var("TEST_ADDR");
            }
        } // Lock is released

        let yaml = yaml_str!("$NOEXISTING_VAR");
        assert!(as_env_sockaddr(&yaml).is_err());

        let yaml = yaml_str!("@invalid_host:8080");
        assert!(as_env_sockaddr(&yaml).is_err());

        let yaml = yaml_str!("@127.0.0.1:8080,127.0.0.1:8081");
        assert!(as_env_sockaddr(&yaml).is_err());

        let yaml = yaml_str!("@nonexistent.domain:8080");
        assert!(as_env_sockaddr(&yaml).is_err());

        let yaml = yaml_str!("invalid");
        assert!(as_env_sockaddr(&yaml).is_err());

        let yaml = Yaml::Integer(12345);
        assert!(as_env_sockaddr(&yaml).is_err());
    }

    #[test]
    fn as_sockaddr_ok() {
        let yaml = yaml_str!("127.0.0.1:8080");
        let addr = as_sockaddr(&yaml).unwrap();
        assert_eq!(addr, "127.0.0.1:8080".parse().unwrap());
    }

    #[test]
    fn as_sockaddr_err() {
        let yaml = yaml_str!("invalid_socket_address");
        assert!(as_sockaddr(&yaml).is_err());

        let yaml = yaml_str!("127.0.0.1");
        assert!(as_sockaddr(&yaml).is_err());

        let yaml = Yaml::Integer(12345);
        assert!(as_sockaddr(&yaml).is_err());
    }

    #[test]
    fn as_weighted_sockaddr_ok() {
        let yaml = yaml_str!("127.0.0.1:8080");
        let result = as_weighted_sockaddr(&yaml).unwrap();
        assert_eq!(*result.inner(), "127.0.0.1:8080".parse().unwrap());
        assert_eq!(result.weight(), 1.0);

        let mut map = yaml_rust::yaml::Hash::new();
        map.insert(yaml_str!("addr"), yaml_str!("192.168.1.1:80"));
        map.insert(yaml_str!("weight"), Yaml::Real("2.5".into()));
        let yaml = Yaml::Hash(map);
        let result = as_weighted_sockaddr(&yaml).unwrap();
        assert_eq!(*result.inner(), "192.168.1.1:80".parse().unwrap());
        assert_eq!(result.weight(), 2.5);

        let mut map = yaml_rust::yaml::Hash::new();
        map.insert(yaml_str!("addr"), yaml_str!("10.0.0.1:443"));
        let yaml = Yaml::Hash(map);
        let result = as_weighted_sockaddr(&yaml).unwrap();
        assert_eq!(*result.inner(), "10.0.0.1:443".parse().unwrap());
        assert_eq!(result.weight(), 1.0);

        let mut map = yaml_rust::yaml::Hash::new();
        map.insert(yaml_str!("addr"), yaml_str!("0.0.0.0:0"));
        map.insert(yaml_str!("weight"), Yaml::Real("0.0".into()));
        let yaml = Yaml::Hash(map);
        let result = as_weighted_sockaddr(&yaml).unwrap();
        assert_eq!(*result.inner(), "0.0.0.0:0".parse().unwrap());
        assert_eq!(result.weight(), 0.0);
    }

    #[test]
    fn as_weighted_sockaddr_err() {
        let yaml = yaml_str!("invalid_address");
        assert!(as_weighted_sockaddr(&yaml).is_err());

        let mut map = yaml_rust::yaml::Hash::new();
        map.insert(yaml_str!("weight"), Yaml::Real("1.0".into()));
        let yaml = Yaml::Hash(map);
        assert!(as_weighted_sockaddr(&yaml).is_err());

        let mut map = yaml_rust::yaml::Hash::new();
        map.insert(yaml_str!("addr"), yaml_str!("127.0.0.1:80"));
        map.insert(yaml_str!("weight"), yaml_str!("invalid"));
        let yaml = Yaml::Hash(map);
        assert!(as_weighted_sockaddr(&yaml).is_err());

        let yaml = Yaml::Integer(12345);
        assert!(as_weighted_sockaddr(&yaml).is_err());
    }

    #[test]
    fn as_ipaddr_ok() {
        let yaml = yaml_str!("127.0.0.1");
        let ip = as_ipaddr(&yaml).unwrap();
        assert_eq!(ip, IpAddr::V4("127.0.0.1".parse().unwrap()));

        let yaml = yaml_str!("::1");
        let ip = as_ipaddr(&yaml).unwrap();
        assert_eq!(ip, IpAddr::V6("::1".parse().unwrap()));
    }

    #[test]
    fn as_ipaddr_err() {
        let yaml = yaml_str!("invalid_ip");
        assert!(as_ipaddr(&yaml).is_err());

        let yaml = yaml_str!("");
        assert!(as_ipaddr(&yaml).is_err());

        let yaml = Yaml::Integer(12345);
        assert!(as_ipaddr(&yaml).is_err());
    }

    #[test]
    fn as_ipv4addr_ok() {
        let yaml = yaml_str!("127.0.0.1");
        let ip = as_ipaddr(&yaml).unwrap();
        assert_eq!(ip, Ipv4Addr::new(127, 0, 0, 1));
    }

    #[test]
    fn as_ipv4addr_err() {
        let yaml = yaml_str!("::1");
        assert!(as_ipv4addr(&yaml).is_err());

        let yaml = yaml_str!("invalid_ip");
        assert!(as_ipaddr(&yaml).is_err());

        let yaml = Yaml::Integer(12345);
        assert!(as_ipaddr(&yaml).is_err());
    }

    #[test]
    fn as_ipv6addr_ok() {
        let yaml = yaml_str!("::1");
        let ip = as_ipv6addr(&yaml).unwrap();
        assert_eq!(ip, Ipv6Addr::new(0, 0, 0, 0, 0, 0, 0, 1));
    }

    #[test]
    fn as_ipv6addr_err() {
        let yaml = yaml_str!("127.0.0.1");
        assert!(as_ipv6addr(&yaml).is_err());

        let yaml = yaml_str!("invalid_ip");
        assert!(as_ipv6addr(&yaml).is_err());

        let yaml = Yaml::Integer(12345);
        assert!(as_ipv6addr(&yaml).is_err());
    }

    #[test]
    #[cfg(feature = "acl-rule")]
    fn as_ip_network_ok() {
        let yaml = yaml_str!("192.168.0.0/24");
        let net = as_ip_network(&yaml).unwrap();
        assert_eq!(
            net,
            IpNetwork::new(Ipv4Addr::new(192, 168, 0, 0), 24).unwrap()
        );

        let yaml = yaml_str!("192.168.0.1");
        let net = as_ip_network(&yaml).unwrap();
        assert_eq!(
            net,
            IpNetwork::new(Ipv4Addr::new(192, 168, 0, 1), 32).unwrap()
        );

        let yaml = yaml_str!("2001:db8::/48");
        let net = as_ip_network(&yaml).unwrap();
        assert_eq!(
            net,
            IpNetwork::new(Ipv6Addr::new(0x2001, 0x0db8, 0, 0, 0, 0, 0, 0), 48).unwrap()
        );

        let yaml = yaml_str!("::1");
        let net = as_ip_network(&yaml).unwrap();
        assert_eq!(
            net,
            IpNetwork::new(Ipv6Addr::new(0, 0, 0, 0, 0, 0, 0, 1), 128).unwrap()
        );
    }

    #[test]
    #[cfg(feature = "acl-rule")]
    fn as_ip_network_err() {
        let yaml = yaml_str!("192.168.0.0/33");
        assert!(as_ip_network(&yaml).is_err());

        let yaml = yaml_str!("::1/129");
        assert!(as_ip_network(&yaml).is_err());

        let yaml = yaml_str!("invalid_ip_network");
        assert!(as_ip_network(&yaml).is_err());

        let yaml = Yaml::Integer(12345);
        assert!(as_ip_network(&yaml).is_err());
    }

    #[test]
    fn as_host_ok() {
        let yaml = yaml_str!("127.0.0.1");
        let host = as_host(&yaml).unwrap();
        assert_eq!(host, Host::Ip(IpAddr::V4(Ipv4Addr::new(127, 0, 0, 1))));

        let yaml = yaml_str!("::1");
        let host = as_host(&yaml).unwrap();
        assert_eq!(
            host,
            Host::Ip(IpAddr::V6(Ipv6Addr::new(0, 0, 0, 0, 0, 0, 0, 1)))
        );

        let yaml = yaml_str!("example.com");
        let host = as_host(&yaml).unwrap();
        assert_eq!(host, Host::Domain("example.com".into()));

        let yaml = yaml_str!("valid domain.com");
        let host = as_host(&yaml).unwrap();
        assert_eq!(host, Host::Domain("valid domain.com".into()));
    }

    #[test]
    fn as_host_err() {
        let yaml = yaml_str!("invalid\u{e000}host");
        assert!(as_host(&yaml).is_err());

        let yaml = Yaml::Integer(12345);
        assert!(as_host(&yaml).is_err());
    }

    #[test]
    fn as_domain_ok() {
        let yaml = yaml_str!("example.com");
        let domain = as_domain(&yaml).unwrap();
        assert_eq!(domain, "example.com".to_string());

        let yaml = yaml_str!("valid domain.com");
        let domain = as_domain(&yaml).unwrap();
        assert_eq!(domain, "valid domain.com".to_string());

        let yaml = yaml_str!("ドメイン.テスト");
        let domain = as_domain(&yaml).unwrap();
        assert_eq!(domain, "xn--eckwd4c7c.xn--zckzah");
    }

    #[test]
    fn as_domain_err() {
        let yaml = yaml_str!("invalid\u{e000}domain");
        assert!(as_domain(&yaml).is_err());

        let yaml = Yaml::Integer(12345);
        assert!(as_domain(&yaml).is_err());
    }

    #[test]
    fn as_url_ok() {
        let yaml = yaml_str!("https://example.com");
        let url = as_url(&yaml).unwrap();
        assert_eq!(url, Url::parse("https://example.com").unwrap());
    }

    #[test]
    fn as_url_err() {
        let yaml = yaml_str!("invalid_url");
        assert!(as_url(&yaml).is_err());

        let yaml = Yaml::Integer(12345);
        assert!(as_url(&yaml).is_err());
    }

    #[test]
    fn as_upstream_addr_ok() {
        let yaml = yaml_str!("example.com:8080");
        let addr = as_upstream_addr(&yaml, 0).unwrap();
        assert_eq!(addr, UpstreamAddr::from_str("example.com:8080").unwrap());

        let yaml = yaml_str!("example.com");
        let addr = as_upstream_addr(&yaml, 80).unwrap();
        assert_eq!(addr, UpstreamAddr::from_str("example.com:80").unwrap());
    }

    #[test]
    fn as_upstream_addr_err() {
        let yaml = yaml_str!("example.com");
        assert!(as_upstream_addr(&yaml, 0).is_err());

        let yaml = yaml_str!("invalid\u{e000}address");
        assert!(as_upstream_addr(&yaml, 80).is_err());

        let yaml = Yaml::Integer(12345);
        assert!(as_upstream_addr(&yaml, 80).is_err());
    }

    #[test]
    fn as_weighted_upstream_addr_ok() {
        let yaml = yaml_str!("example.com:8080");
        let result = as_weighted_upstream_addr(&yaml, 0).unwrap();
        assert_eq!(
            *result.inner(),
            UpstreamAddr::from_str("example.com:8080").unwrap()
        );
        assert_eq!(result.weight(), WeightedUpstreamAddr::DEFAULT_WEIGHT);

        let yaml = yaml_str!("example.com");
        let result = as_weighted_upstream_addr(&yaml, 80).unwrap();
        assert_eq!(
            *result.inner(),
            UpstreamAddr::from_str("example.com:80").unwrap()
        );
        assert_eq!(result.weight(), WeightedUpstreamAddr::DEFAULT_WEIGHT);

        let mut map = yaml_rust::yaml::Hash::new();
        map.insert(yaml_str!("addr"), yaml_str!("test.com:443"));
        map.insert(yaml_str!("weight"), Yaml::Real("2.5".into()));
        let yaml = Yaml::Hash(map);
        let result = as_weighted_upstream_addr(&yaml, 0).unwrap();
        assert_eq!(
            *result.inner(),
            UpstreamAddr::from_str("test.com:443").unwrap()
        );
        assert_eq!(result.weight(), 2.5);

        let mut map = yaml_rust::yaml::Hash::new();
        map.insert(yaml_str!("addr"), yaml_str!("api.example.com:8080"));
        let yaml = Yaml::Hash(map);
        let result = as_weighted_upstream_addr(&yaml, 0).unwrap();
        assert_eq!(
            *result.inner(),
            UpstreamAddr::from_str("api.example.com:8080").unwrap()
        );
        assert_eq!(result.weight(), WeightedUpstreamAddr::DEFAULT_WEIGHT);

        let mut map = yaml_rust::yaml::Hash::new();
        map.insert(yaml_str!("address"), yaml_str!("example.com:9090"));
        let yaml = Yaml::Hash(map);
        let result = as_weighted_upstream_addr(&yaml, 0).unwrap();
        assert_eq!(
            *result.inner(),
            UpstreamAddr::from_str("example.com:9090").unwrap()
        );
        assert_eq!(result.weight(), WeightedUpstreamAddr::DEFAULT_WEIGHT);
    }

    #[test]
    fn as_weighted_upstream_addr_err() {
        let mut map = yaml_rust::yaml::Hash::new();
        map.insert(yaml_str!("invalid_key"), yaml_str!("test.com:443"));
        let yaml = Yaml::Hash(map);
        assert!(as_weighted_upstream_addr(&yaml, 0).is_err());

        let yaml = yaml_str!("invalid\u{e000}address");
        assert!(as_weighted_upstream_addr(&yaml, 80).is_err());

        let mut map = yaml_rust::yaml::Hash::new();
        map.insert(yaml_str!("weight"), Yaml::Real("1.0".into()));
        let yaml = Yaml::Hash(map);
        assert!(as_weighted_upstream_addr(&yaml, 80).is_err());

        let mut map = yaml_rust::yaml::Hash::new();
        map.insert(yaml_str!("addr"), yaml_str!("127.0.0.1:80"));
        map.insert(yaml_str!("weight"), yaml_str!("invalid"));
        let yaml = Yaml::Hash(map);
        assert!(as_weighted_upstream_addr(&yaml, 0).is_err());

        let yaml = Yaml::Integer(12345);
        assert!(as_weighted_upstream_addr(&yaml, 80).is_err());

        let mut map = yaml_rust::yaml::Hash::new();
        map.insert(yaml_str!("addr"), yaml_str!(""));
        let yaml = Yaml::Hash(map);
        assert!(as_weighted_upstream_addr(&yaml, 80).is_err());
    }
}
