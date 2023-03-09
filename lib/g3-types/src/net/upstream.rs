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

use std::borrow::Cow;
use std::fmt;
use std::hash::Hash;
use std::net::{IpAddr, Ipv4Addr, Ipv6Addr, SocketAddrV6};
use std::str::FromStr;

use anyhow::anyhow;
use url::Url;

use crate::collection::WeightedValue;
use crate::net::Host;

#[derive(Eq, PartialEq, Hash)]
pub enum UpstreamHostRef<'a> {
    Ip(IpAddr),
    Domain(&'a str),
}

#[derive(Debug, Clone, Eq, PartialEq, Hash)]
pub struct UpstreamAddr {
    host: Host,
    port: u16,
}

impl UpstreamAddr {
    pub fn new(host: Host, port: u16) -> Self {
        UpstreamAddr { host, port }
    }

    pub fn empty() -> Self {
        UpstreamAddr {
            host: Host::empty(),
            port: 0,
        }
    }

    #[inline]
    pub fn is_empty(&self) -> bool {
        self.host.is_empty()
    }

    #[inline]
    pub fn host(&self) -> &Host {
        &self.host
    }

    pub fn host_str(&self) -> Cow<str> {
        match &self.host {
            Host::Domain(s) => Cow::Borrowed(s),
            Host::Ip(ip) => Cow::Owned(ip.to_string()),
        }
    }

    #[inline]
    pub fn host_eq(&self, other: &Self) -> bool {
        self.host.eq(&other.host)
    }

    #[inline]
    pub fn port(&self) -> u16 {
        self.port
    }

    pub fn set_port(&mut self, port: u16) {
        self.port = port;
    }

    #[inline]
    pub fn from_ip_and_port(ip: IpAddr, port: u16) -> Self {
        UpstreamAddr {
            host: Host::Ip(ip),
            port,
        }
    }

    fn from_domain_str_and_port(domain: &str, port: u16) -> anyhow::Result<Self> {
        let host = Host::from_domain_str(domain)?;
        Ok(UpstreamAddr { host, port })
    }

    pub fn from_host_str_and_port(host: &str, port: u16) -> anyhow::Result<Self> {
        let host = Host::from_str(host)?;
        Ok(UpstreamAddr { host, port })
    }

    fn from_full_domain_str(s: &str) -> anyhow::Result<Self> {
        let mut port = 0;
        let domain = if let Some(i) = memchr::memchr(b':', s.as_bytes()) {
            port = u16::from_str(&s[i + 1..]).map_err(|_| anyhow!("invalid port"))?;
            &s[..i]
        } else {
            s
        };

        UpstreamAddr::from_domain_str_and_port(domain, port)
    }

    #[inline]
    fn from_maybe_mapped_ip6(ip6: Ipv6Addr, port: u16) -> Self {
        let host = Host::from_maybe_mapped_ip6(ip6);
        UpstreamAddr { host, port }
    }

    #[inline]
    pub(crate) fn from_url_host_and_port(host: url::Host, port: u16) -> Self {
        UpstreamAddr {
            host: host.into(),
            port,
        }
    }
}

impl TryFrom<&Url> for UpstreamAddr {
    type Error = anyhow::Error;

    fn try_from(u: &Url) -> Result<Self, Self::Error> {
        if let Some(host) = u.host() {
            let port = u
                .port_or_known_default()
                .ok_or_else(|| anyhow!("unable to detect port in this url"))?;
            Ok(UpstreamAddr::from_url_host_and_port(host.to_owned(), port))
        } else {
            Err(anyhow!("no host found in this url"))
        }
    }
}

impl FromStr for UpstreamAddr {
    type Err = anyhow::Error;

    fn from_str(s: &str) -> anyhow::Result<Self> {
        if s.is_empty() {
            return Err(anyhow!("empty str"));
        }
        match s.as_bytes()[0] {
            b'[' => {
                let pos_last = s.len() - 1;
                match s.as_bytes()[pos_last] {
                    b']' => {
                        let ip6 = Ipv6Addr::from_str(&s[1..pos_last])
                            .map_err(|_| anyhow!("invalid ipv6 ip in squared brackets"))?;
                        Ok(UpstreamAddr::from_maybe_mapped_ip6(ip6, 0))
                    }
                    b'0'..=b'9' => {
                        let addr6 =
                            SocketAddrV6::from_str(s).map_err(|_| anyhow!("invalid ipv6 addr"))?;
                        Ok(UpstreamAddr::from_maybe_mapped_ip6(
                            *addr6.ip(),
                            addr6.port(),
                        ))
                    }
                    _ => Err(anyhow!("invalid ipv6 ip or addr")),
                }
            }
            b':' => {
                let ip6 = Ipv6Addr::from_str(s).map_err(|_| anyhow!("invalid ipv6 ip"))?;
                Ok(UpstreamAddr::from_maybe_mapped_ip6(ip6, 0))
            }
            b'0'..=b'9' => {
                if let Some(i) = memchr::memchr(b':', s.as_bytes()) {
                    if memchr::memchr(b':', &s.as_bytes()[i + 1..]).is_some() {
                        let ip6 = Ipv6Addr::from_str(s).map_err(|_| anyhow!("invalid ipv6 ip"))?;
                        Ok(UpstreamAddr::from_maybe_mapped_ip6(ip6, 0))
                    } else {
                        let port =
                            u16::from_str(&s[i + 1..]).map_err(|_| anyhow!("invalid port"))?;
                        if let Ok(ip4) = Ipv4Addr::from_str(&s[0..i]) {
                            Ok(UpstreamAddr::from_ip_and_port(IpAddr::V4(ip4), port))
                        } else {
                            UpstreamAddr::from_domain_str_and_port(&s[0..i], port)
                        }
                    }
                } else if let Ok(ip4) = Ipv4Addr::from_str(s) {
                    Ok(UpstreamAddr::from_ip_and_port(IpAddr::V4(ip4), 0))
                } else {
                    UpstreamAddr::from_domain_str_and_port(s, 0)
                }
            }
            b'a'..=b'f' | b'A'..=b'F' => {
                if let Ok(ip6) = Ipv6Addr::from_str(s) {
                    // won't be ipv4 mapped
                    Ok(UpstreamAddr::from_ip_and_port(IpAddr::V6(ip6), 0))
                } else {
                    UpstreamAddr::from_full_domain_str(s)
                }
            }
            _ => UpstreamAddr::from_full_domain_str(s),
        }
    }
}

impl fmt::Display for UpstreamAddr {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match &self.host {
            Host::Ip(IpAddr::V4(ip4)) => write!(f, "{ip4}:{}", self.port),
            Host::Ip(IpAddr::V6(ip6)) => write!(f, "[{ip6}]:{}", self.port),
            Host::Domain(domain) => write!(f, "{domain}:{}", self.port),
        }
    }
}

pub type WeightedUpstreamAddr = WeightedValue<UpstreamAddr>;

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parse_err() {
        assert!(UpstreamAddr::from_str("").is_err());
    }

    #[test]
    fn parse_ok() {
        let mut ipv4 = UpstreamAddr::from_str("127.0.0.1:8080").unwrap();
        assert_eq!(
            ipv4,
            UpstreamAddr::from_ip_and_port(IpAddr::from_str("127.0.0.1").unwrap(), 8080)
        );

        ipv4 = UpstreamAddr::from_str("127.0.0.1").unwrap();
        assert_eq!(
            ipv4,
            UpstreamAddr::from_ip_and_port(IpAddr::from_str("127.0.0.1").unwrap(), 0)
        );

        let mut ipv6 = UpstreamAddr::from_str("[2001:db8::1]:8080").unwrap();
        assert_eq!(
            ipv6,
            UpstreamAddr::from_ip_and_port(IpAddr::from_str("2001:db8::1").unwrap(), 8080)
        );

        ipv6 = UpstreamAddr::from_str("[2001:db8::1]").unwrap();
        ipv6.set_port(80);
        assert_eq!(
            ipv6,
            UpstreamAddr::from_ip_and_port(IpAddr::from_str("2001:db8::1").unwrap(), 80)
        );

        let mut ipv6mapped = UpstreamAddr::from_str("[::ffff:192.168.89.9]:8080").unwrap();
        assert_eq!(
            ipv6mapped,
            UpstreamAddr::from_ip_and_port(IpAddr::from_str("192.168.89.9").unwrap(), 8080)
        );

        ipv6mapped = UpstreamAddr::from_str("[::ffff:192.168.89.9]").unwrap();
        ipv6mapped.set_port(80);
        assert_eq!(
            ipv6mapped,
            UpstreamAddr::from_ip_and_port(IpAddr::from_str("192.168.89.9").unwrap(), 80)
        );

        let mut domain = UpstreamAddr::from_str("www.haha哈哈.com:8080").unwrap();
        assert_eq!(
            domain,
            UpstreamAddr::from_domain_str_and_port("www.xn--haha-oc2ga.com", 8080).unwrap()
        );

        ipv4 = UpstreamAddr::from_host_str_and_port("127.0.0.1", 8080).unwrap();
        assert_eq!(
            ipv4,
            UpstreamAddr::from_ip_and_port(IpAddr::from_str("127.0.0.1").unwrap(), 8080)
        );

        ipv6 = UpstreamAddr::from_host_str_and_port("2001:db8::1", 8080).unwrap();
        assert_eq!(
            ipv6,
            UpstreamAddr::from_ip_and_port(IpAddr::from_str("2001:db8::1").unwrap(), 8080)
        );

        ipv6 = UpstreamAddr::from_host_str_and_port("[2001:db8::1]", 8080).unwrap();
        assert_eq!(
            ipv6,
            UpstreamAddr::from_ip_and_port(IpAddr::from_str("2001:db8::1").unwrap(), 8080)
        );

        ipv6mapped = UpstreamAddr::from_host_str_and_port("::ffff:192.168.89.9", 8080).unwrap();
        assert_eq!(
            ipv6mapped,
            UpstreamAddr::from_ip_and_port(IpAddr::from_str("192.168.89.9").unwrap(), 8080)
        );

        ipv6mapped = UpstreamAddr::from_host_str_and_port("[::ffff:192.168.89.9]", 8080).unwrap();
        assert_eq!(
            ipv6mapped,
            UpstreamAddr::from_ip_and_port(IpAddr::from_str("192.168.89.9").unwrap(), 8080)
        );

        domain = UpstreamAddr::from_host_str_and_port("www.haha哈哈.com", 8080).unwrap();
        assert_eq!(
            domain,
            UpstreamAddr::from_domain_str_and_port("www.xn--haha-oc2ga.com", 8080).unwrap()
        );
    }

    #[test]
    fn parse_url_ok() {
        use url::Url;

        let url = Url::parse("http://[2001:db8::1]/p?q=1").unwrap();
        let ipv6 = UpstreamAddr::try_from(&url).unwrap();
        assert_eq!(
            ipv6,
            UpstreamAddr::from_ip_and_port(IpAddr::from_str("2001:db8::1").unwrap(), 80)
        );

        let url = Url::parse("http://[2001:db8::1]:8080/p?q=1").unwrap();
        let ipv6 = UpstreamAddr::try_from(&url).unwrap();
        assert_eq!(
            ipv6,
            UpstreamAddr::from_ip_and_port(IpAddr::from_str("2001:db8::1").unwrap(), 8080)
        );

        let url = Url::parse("http://www.haha哈哈.com/").unwrap();
        let domain = UpstreamAddr::try_from(&url).unwrap();
        assert_eq!(
            domain,
            UpstreamAddr::from_domain_str_and_port("www.xn--haha-oc2ga.com", 80).unwrap()
        );
    }
}
