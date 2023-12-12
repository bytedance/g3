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

use std::net::SocketAddr;
use std::str::FromStr;

use anyhow::{anyhow, Context};
use yaml_rust::Yaml;

use g3_types::net::{UdpListenConfig, UdpMiscSockOpts};

pub fn as_udp_misc_sock_opts(v: &Yaml) -> anyhow::Result<UdpMiscSockOpts> {
    let mut config = UdpMiscSockOpts::default();

    if let Yaml::Hash(map) = v {
        crate::foreach_kv(map, |k, v| match crate::key::normalize(k).as_str() {
            "time_to_live" | "ttl" => {
                let ttl =
                    crate::value::as_u32(v).context(format!("invalid u32 value for key {k}"))?;
                config.time_to_live = Some(ttl);
                Ok(())
            }
            "type_of_service" | "tos" => {
                let tos =
                    crate::value::as_u8(v).context(format!("invalid u8 value for key {k}"))?;
                config.type_of_service = Some(tos);
                Ok(())
            }
            "netfilter_mark" | "mark" => {
                let mark =
                    crate::value::as_u32(v).context(format!("invalid u32 value for key {k}"))?;
                config.netfilter_mark = Some(mark);
                Ok(())
            }
            _ => Err(anyhow!("invalid key {k}")),
        })?;

        Ok(config)
    } else {
        Err(anyhow!(
            "yaml value type for 'UdpMiscSockOpts' should be 'map'"
        ))
    }
}

fn set_udp_listen_scale(config: &mut UdpListenConfig, v: &Yaml) -> anyhow::Result<()> {
    match v {
        Yaml::String(s) => {
            if s.ends_with('%') {
                let Ok(v) = f64::from_str(&s[..s.len() - 1]) else {
                    return Err(anyhow!("invalid percentage value {s}"));
                };
                config
                    .set_scale(v / 100.0)
                    .context(format!("unsupported percentage value {s}"))
            } else if let Some((n, d)) = s.split_once('/') {
                let Ok(n) = usize::from_str(n.trim()) else {
                    return Err(anyhow!("invalid fractional value {s}: invalid numerator"))?;
                };
                let Ok(d) = usize::from_str(d.trim()) else {
                    return Err(anyhow!("invalid fractional value {s}: invalid denominator"))?;
                };
                config.set_fraction_scale(n, d);
                Ok(())
            } else {
                let Ok(v) = f64::from_str(s) else {
                    return Err(anyhow!("invalid float value: {s}"));
                };
                config
                    .set_scale(v)
                    .context(format!("unsupported float value {s}"))
            }
        }
        Yaml::Integer(i) => config
            .set_scale(*i as f64)
            .context(format!("unsupported integer value {i}")),
        Yaml::Real(s) => {
            let Ok(v) = f64::from_str(s) else {
                return Err(anyhow!("invalid float value: {s}"));
            };
            config
                .set_scale(v)
                .context(format!("unsupported float value {s}"))
        }
        _ => Err(anyhow!(
            "yaml value type for udp listen scale value should be 'str' or 'float'"
        )),
    }
}

pub fn as_udp_listen_config(value: &Yaml) -> anyhow::Result<UdpListenConfig> {
    let mut config = UdpListenConfig::default();

    match value {
        Yaml::Integer(i) => {
            let port = u16::try_from(*i).map_err(|e| anyhow!("out of range u16 value: {e}"))?;
            config.set_port(port);
        }
        Yaml::String(s) => {
            let addr =
                SocketAddr::from_str(s).map_err(|e| anyhow!("invalid socket address: {e}"))?;
            config.set_socket_address(addr);
        }
        Yaml::Hash(map) => {
            crate::foreach_kv(map, |k, v| match crate::key::normalize(k).as_str() {
                "addr" | "address" => {
                    let addr = crate::value::as_env_sockaddr(v).context(format!(
                        "invalid udp listen socket address value for key {k}"
                    ))?;
                    config.set_socket_address(addr);
                    Ok(())
                }
                "ipv6only" | "ipv6_only" => {
                    let ipv6only = crate::value::as_bool(v)
                        .context(format!("invalid bool value for key {k}"))?;
                    config.set_ipv6_only(ipv6only);
                    Ok(())
                }
                "socket_buffer" => {
                    let buf_conf = crate::value::as_socket_buffer_config(v)
                        .context(format!("invalid socket buffer config value for key {k}"))?;
                    config.set_socket_buffer(buf_conf);
                    Ok(())
                }
                "socket_misc_opts" => {
                    let misc_opts = as_udp_misc_sock_opts(v)
                        .context(format!("invalid udp socket misc opts value for key {k}"))?;
                    config.set_socket_misc_opts(misc_opts);
                    Ok(())
                }
                "instance" | "instance_count" => {
                    let instance = crate::value::as_usize(v)
                        .context(format!("invalid usize value for key {k}"))?;
                    config.set_instance(instance);
                    Ok(())
                }
                "scale" => set_udp_listen_scale(&mut config, v)
                    .context(format!("invalid scale value for key {k}")),
                _ => Err(anyhow!("invalid key {k}")),
            })?;
        }
        _ => return Err(anyhow!("invalid value type")),
    }

    config.check()?;
    Ok(config)
}
