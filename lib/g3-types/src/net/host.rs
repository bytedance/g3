/*
 * SPDX-License-Identifier: Apache-2.0
 * Copyright 2023-2025 ByteDance and/or its affiliates.
 */

use std::fmt;
use std::net::{IpAddr, Ipv4Addr, Ipv6Addr};
use std::str::FromStr;
use std::sync::Arc;

use anyhow::anyhow;
#[cfg(feature = "rustls")]
use rustls_pki_types::ServerName;

#[derive(Debug, Clone, Eq, PartialEq, Hash)]
pub enum Host {
    Ip(IpAddr),
    Domain(Arc<str>),
}

impl Host {
    pub const fn empty() -> Self {
        Host::Ip(IpAddr::V4(Ipv4Addr::new(0, 0, 0, 0)))
    }

    pub const fn localhost_v4() -> Self {
        Host::Ip(IpAddr::V4(Ipv4Addr::LOCALHOST))
    }

    pub fn is_empty(&self) -> bool {
        match self {
            Host::Ip(ip) => ip.is_unspecified(),
            Host::Domain(domain) => domain.is_empty(),
        }
    }

    pub(crate) fn from_maybe_mapped_ip6(ip6: Ipv6Addr) -> Self {
        if let Some(ip4) = ip6.to_ipv4_mapped() {
            Host::Ip(IpAddr::V4(ip4))
        } else {
            Host::Ip(IpAddr::V6(ip6))
        }
    }

    pub(crate) fn from_domain_str(domain: &str) -> anyhow::Result<Self> {
        let domain = idna::domain_to_ascii(domain).map_err(|e| anyhow!("invalid domain: {e}"))?;
        Ok(Host::Domain(Arc::from(domain)))
    }

    pub fn parse_smtp_host_address(buf: &[u8]) -> Option<Self> {
        if buf.is_empty() {
            return None;
        }
        if buf[0] == b'[' {
            let end = buf.len() - 1;
            if buf[end] != b']' {
                return None;
            }
            let Ok(s) = std::str::from_utf8(&buf[1..end]) else {
                return None;
            };
            Ipv4Addr::from_str(s)
                .map(|v4| Host::Ip(IpAddr::V4(v4)))
                .ok()
        } else if let Some(d) = memchr::memchr(b':', buf) {
            match &buf[0..d] {
                b"Ipv6" => {
                    let Ok(s) = std::str::from_utf8(&buf[d + 1..]) else {
                        return None;
                    };
                    Ipv6Addr::from_str(s)
                        .map(|v6| Host::Ip(IpAddr::V6(v6)))
                        .ok()
                }
                _ => None,
            }
        } else {
            let Ok(s) = std::str::from_utf8(buf) else {
                return None;
            };
            Host::from_domain_str(s).ok()
        }
    }
}

impl fmt::Display for Host {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Host::Ip(ip) => write!(f, "{ip}"),
            Host::Domain(domain) => f.write_str(domain),
        }
    }
}

impl From<url::Host> for Host {
    fn from(v: url::Host) -> Self {
        match v {
            url::Host::Ipv4(ip4) => Host::Ip(IpAddr::V4(ip4)),
            url::Host::Ipv6(ip6) => Host::Ip(IpAddr::V6(ip6)),
            url::Host::Domain(domain) => Host::Domain(Arc::from(domain)),
        }
    }
}

impl FromStr for Host {
    type Err = anyhow::Error;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        if s.is_empty() {
            return Err(anyhow!("empty string"));
        }
        match s.as_bytes()[0] {
            b'[' => {
                let pos_last = s.len() - 1;
                if s.as_bytes()[pos_last] == b']' {
                    if let Ok(ip6) = Ipv6Addr::from_str(&s[1..pos_last]) {
                        return Ok(Host::from_maybe_mapped_ip6(ip6));
                    }
                }
                return Err(anyhow!("invalid ipv6 ip in squared brackets"));
            }
            b':' => {
                return if let Ok(ip6) = Ipv6Addr::from_str(s) {
                    Ok(Host::from_maybe_mapped_ip6(ip6))
                } else {
                    Err(anyhow!("invalid ipv6 ip"))
                };
            }
            b'0'..=b'9' => {
                if let Ok(ip) = IpAddr::from_str(s) {
                    return match ip {
                        IpAddr::V4(_) => Ok(Host::Ip(ip)),
                        IpAddr::V6(ip6) => Ok(Host::from_maybe_mapped_ip6(ip6)),
                    };
                }
            }
            b'a'..=b'f' | b'A'..=b'F' => {
                if let Ok(ip6) = Ipv6Addr::from_str(s) {
                    // won't be ipv4 mapped
                    return Ok(Host::Ip(IpAddr::V6(ip6)));
                }
            }
            _ => {}
        }

        Host::from_domain_str(s)
    }
}

#[cfg(feature = "rustls")]
impl TryFrom<&Host> for ServerName<'static> {
    type Error = std::io::Error;

    fn try_from(value: &Host) -> Result<Self, Self::Error> {
        use std::io;

        match value {
            Host::Ip(ip) => Ok(ServerName::IpAddress((*ip).into())),
            Host::Domain(domain) => ServerName::try_from(domain.as_ref())
                .map(|r| r.to_owned())
                .map_err(|e| io::Error::new(io::ErrorKind::InvalidInput, e)),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn smtp_address() {
        let host = Host::parse_smtp_host_address(b"www.example.net").unwrap();
        assert_eq!(host, Host::Domain(Arc::from("www.example.net")));

        let host = Host::parse_smtp_host_address(b"[123.255.37.2]").unwrap();
        assert_eq!(host, Host::Ip(IpAddr::from_str("123.255.37.2").unwrap()));

        let host = Host::parse_smtp_host_address(b"Ipv6:2001:db8::1").unwrap();
        assert_eq!(host, Host::Ip(IpAddr::from_str("2001:db8::1").unwrap()));
    }
}
