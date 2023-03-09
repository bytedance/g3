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

use std::net::IpAddr;
use std::str::FromStr;

use anyhow::{anyhow, Context};
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
            Ok(Host::Domain(domain))
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
