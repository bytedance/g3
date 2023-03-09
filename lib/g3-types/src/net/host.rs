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

use std::fmt;
use std::net::{IpAddr, Ipv4Addr, Ipv6Addr};
use std::str::FromStr;

use anyhow::anyhow;

#[derive(Debug, Clone, Eq, PartialEq, Hash)]
pub enum Host {
    Ip(IpAddr),
    Domain(String),
}

impl Host {
    pub fn empty() -> Self {
        Host::Ip(IpAddr::V4(Ipv4Addr::new(0, 0, 0, 0)))
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
        Ok(Host::Domain(domain))
    }
}

impl fmt::Display for Host {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Host::Ip(ip) => write!(f, "{ip}"),
            Host::Domain(domain) => write!(f, "{domain}"),
        }
    }
}

impl From<url::Host> for Host {
    fn from(v: url::Host) -> Self {
        match v {
            url::Host::Ipv4(ip4) => Host::Ip(IpAddr::V4(ip4)),
            url::Host::Ipv6(ip6) => Host::Ip(IpAddr::V6(ip6)),
            url::Host::Domain(domain) => Host::Domain(domain),
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
                }
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

#[cfg(feature = "rustls-config")]
impl TryFrom<&Host> for rustls::ServerName {
    type Error = std::io::Error;

    fn try_from(value: &Host) -> Result<Self, Self::Error> {
        use std::io;

        match value {
            Host::Ip(_ip) => Err(io::Error::new(
                io::ErrorKind::Other,
                "ip verification is not supported",
            )),
            Host::Domain(domain) => rustls::ServerName::try_from(domain.as_str())
                .map_err(|e| io::Error::new(io::ErrorKind::InvalidInput, e)),
        }
    }
}
