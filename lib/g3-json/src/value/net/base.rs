/*
 * SPDX-License-Identifier: Apache-2.0
 * Copyright 2023-2025 ByteDance and/or its affiliates.
 */

use std::net::IpAddr;
use std::str::FromStr;

use anyhow::{Context, anyhow};
use serde_json::Value;

#[cfg(feature = "acl-rule")]
use ip_network::IpNetwork;

use g3_types::net::{EgressArea, Host, UpstreamAddr};

pub fn as_ipaddr(v: &Value) -> anyhow::Result<IpAddr> {
    match v {
        Value::String(s) => {
            let ip = IpAddr::from_str(s).map_err(|e| anyhow!("invalid ip address string: {e}"))?;
            Ok(ip)
        }
        _ => Err(anyhow!("json value type for 'IpAddr' should be 'string'")),
    }
}

#[cfg(feature = "acl-rule")]
pub fn as_ip_network(v: &Value) -> anyhow::Result<IpNetwork> {
    if let Value::String(s) = v {
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
            "json value type for 'IpNetwork' should be 'string'"
        ))
    }
}

pub fn as_host(v: &Value) -> anyhow::Result<Host> {
    if let Value::String(value) = v {
        if let Ok(ip) = IpAddr::from_str(value) {
            Ok(Host::Ip(ip))
        } else {
            // allow more than domain_to_ascii_strict chars
            let domain = idna::domain_to_ascii(value).map_err(|e| anyhow!("invalid host: {e}"))?;
            Ok(Host::Domain(domain.into()))
        }
    } else {
        Err(anyhow!("json value type for 'Host' should be 'string'"))
    }
}

pub fn as_domain(v: &Value) -> anyhow::Result<String> {
    if let Value::String(s) = v {
        // allow more than domain_to_ascii_strict chars
        let domain = idna::domain_to_ascii(s).map_err(|e| anyhow!("invalid domain: {e}"))?;
        Ok(domain)
    } else {
        Err(anyhow!("json value type for 'Domain' should be 'string'"))
    }
}

pub fn as_upstream_addr(v: &Value) -> anyhow::Result<UpstreamAddr> {
    if let Value::String(s) = v {
        let addr = UpstreamAddr::from_str(s).context("invalid upstream addr string")?;
        Ok(addr)
    } else {
        Err(anyhow!(
            "json value type for upstream addr should be 'string'"
        ))
    }
}

pub fn as_egress_area(v: &Value) -> anyhow::Result<EgressArea> {
    if let Value::String(s) = v {
        EgressArea::from_str(s).map_err(|_| anyhow!("invalid egress area string"))
    } else {
        Err(anyhow!(
            "json value type for 'EgressArea' should be 'string'"
        ))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;
    use std::net::{Ipv4Addr, Ipv6Addr};

    #[test]
    fn as_ipaddr_ok() {
        // valid IPv4
        let ipv4 = json!("192.168.1.1");
        assert_eq!(
            as_ipaddr(&ipv4).unwrap(),
            IpAddr::V4(Ipv4Addr::new(192, 168, 1, 1))
        );

        // valid IPv6
        let ipv6 = json!("::1");
        assert_eq!(
            as_ipaddr(&ipv6).unwrap(),
            IpAddr::V6(Ipv6Addr::new(0, 0, 0, 0, 0, 0, 0, 1))
        );
    }

    #[test]
    fn as_ipaddr_err() {
        // invalid IP format
        let invalid_ip = json!("256.256.256.256");
        assert!(as_ipaddr(&invalid_ip).is_err());

        // non-string type
        let non_string = json!(12345);
        assert!(as_ipaddr(&non_string).is_err());
    }

    #[test]
    #[cfg(feature = "acl-rule")]
    fn as_ip_network_ok() {
        // CIDR format
        let cidr = json!("192.168.1.0/24");
        let network = as_ip_network(&cidr).unwrap();
        assert_eq!(network.to_string(), "192.168.1.0/24");

        // valid IPv4 address
        let single_ipv4 = json!("10.0.0.1");
        let network = as_ip_network(&single_ipv4).unwrap();
        assert_eq!(network.to_string(), "10.0.0.1/32");

        // valid IPv6 address
        let single_ipv6 = json!("::1");
        let network = as_ip_network(&single_ipv6).unwrap();
        assert_eq!(network.to_string(), "::1/128");
    }

    #[test]
    #[cfg(feature = "acl-rule")]
    fn as_ip_network_err() {
        // invalid network format
        let invalid = json!("invalid_network");
        assert!(as_ip_network(&invalid).is_err());

        // non-string type
        let non_string = json!(true);
        assert!(as_ip_network(&non_string).is_err());
    }

    #[test]
    fn as_host_ok() {
        // IP host
        let ip_host = json!("192.168.1.1");
        assert_eq!(
            as_host(&ip_host).unwrap(),
            Host::Ip(IpAddr::V4(Ipv4Addr::new(192, 168, 1, 1)))
        );

        // domain host
        let domain_host = json!("example.com");
        assert_eq!(
            as_host(&domain_host).unwrap(),
            Host::Domain("example.com".into())
        );

        // IDN host
        let idn_host = json!("例子.测试");
        assert_eq!(
            as_host(&idn_host).unwrap(),
            Host::Domain("xn--fsqu00a.xn--0zwm56d".into())
        );
    }

    #[test]
    fn as_host_err() {
        // invalid hostname
        let invalid_host = json!("-invalid-\u{e000}.com");
        assert!(as_host(&invalid_host).is_err());

        // non-string type
        let non_string = json!(vec![1, 2, 3]);
        assert!(as_host(&non_string).is_err());
    }

    #[test]
    fn as_domain_ok() {
        // valid domain
        let domain = json!("example.com");
        assert_eq!(as_domain(&domain).unwrap(), "example.com");

        // IDN conversion
        let idn = json!("例子.测试");
        assert_eq!(as_domain(&idn).unwrap(), "xn--fsqu00a.xn--0zwm56d");
    }

    #[test]
    fn as_domain_err() {
        // invalid domain
        let invalid_domain = json!("invalid\u{e000}domain");
        assert!(as_domain(&invalid_domain).is_err());

        // non-string type
        let non_string = json!(42);
        assert!(as_domain(&non_string).is_err());
    }

    #[test]
    fn as_upstream_addr_ok() {
        // IPv4 address
        let ipv4 = json!("127.0.0.1:8080");
        let addr = as_upstream_addr(&ipv4).unwrap();
        assert_eq!(
            addr.host(),
            &Host::Ip(IpAddr::V4(Ipv4Addr::new(127, 0, 0, 1)))
        );
        assert_eq!(addr.port(), 8080);

        // IPv6 address
        let ipv6 = json!("[2001:db8::1]:8080");
        let addr = as_upstream_addr(&ipv6).unwrap();
        assert_eq!(
            addr.host(),
            &Host::Ip(IpAddr::V6(Ipv6Addr::new(0x2001, 0xdb8, 0, 0, 0, 0, 0, 1)))
        );
        assert_eq!(addr.port(), 8080);

        // domain address
        let domain = json!("example.com:443");
        let addr = as_upstream_addr(&domain).unwrap();
        assert_eq!(addr.host(), &Host::Domain("example.com".into()));
        assert_eq!(addr.port(), 443);
    }

    #[test]
    fn as_upstream_addr_err() {
        // invalid format
        let invalid_format = json!("invalid\u{e000}address");
        assert!(as_upstream_addr(&invalid_format).is_err());

        // invalid port number
        let invalid_port = json!("example.com:99999");
        assert!(as_upstream_addr(&invalid_port).is_err());
    }

    #[test]
    fn as_egress_area_ok() {
        // single-level area
        let single = json!("area1");
        assert_eq!(as_egress_area(&single).unwrap().to_string(), "area1");

        // multi-level area
        let multi = json!("area1/area2/area3");
        assert_eq!(
            as_egress_area(&multi).unwrap().to_string(),
            "area1/area2/area3"
        );

        // with spaces
        let with_spaces = json!("  area1 / area2  ");
        assert_eq!(
            as_egress_area(&with_spaces).unwrap().to_string(),
            "area1 / area2"
        );
    }

    #[test]
    fn as_egress_area_err() {
        // empty area
        let empty = json!("");
        assert!(as_egress_area(&empty).is_err());

        // slashes only
        let slashes = json!("///");
        assert!(as_egress_area(&slashes).is_err());

        // non-string type
        let non_string = json!(123);
        assert!(as_egress_area(&non_string).is_err());
    }
}
