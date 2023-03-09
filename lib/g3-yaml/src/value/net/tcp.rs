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

use std::convert::TryFrom;
use std::net::SocketAddr;
use std::str::FromStr;

use anyhow::{anyhow, Context};
use yaml_rust::Yaml;

use g3_types::net::{
    HappyEyeballsConfig, TcpConnectConfig, TcpKeepAliveConfig, TcpListenConfig, TcpMiscSockOpts,
};

fn set_tcp_listen_scale(config: &mut TcpListenConfig, v: &Yaml) -> anyhow::Result<()> {
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
            "yaml value type for tcp listen scale value should be 'str' or 'float'"
        )),
    }
}

pub fn as_tcp_listen_config(value: &Yaml) -> anyhow::Result<TcpListenConfig> {
    let mut config = TcpListenConfig::default();

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
                    let addr = crate::value::as_sockaddr(v)
                        .context(format!("invalid SocketAddr value for key {k}"))?;
                    config.set_socket_address(addr);
                    Ok(())
                }
                "backlog" => {
                    let backlog = crate::value::as_u32(v)
                        .context(format!("invalid u32 value for key {k}"))?;
                    config.set_backlog(backlog);
                    Ok(())
                }
                "ipv6only" | "ipv6_only" => {
                    let ipv6only = crate::value::as_bool(v)
                        .context(format!("invalid bool value for key {k}"))?;
                    config.set_ipv6_only(ipv6only);
                    Ok(())
                }
                "instance" | "instance_count" => {
                    let instance = crate::value::as_usize(v)
                        .context(format!("invalid usize value for key {k}"))?;
                    config.set_instance(instance);
                    Ok(())
                }
                "scale" => set_tcp_listen_scale(&mut config, v)
                    .context(format!("invalid scale value for key {k}")),
                _ => Err(anyhow!("invalid key {k}")),
            })?;
        }
        _ => return Err(anyhow!("invalid value type")),
    }

    config.check()?;
    Ok(config)
}

pub fn as_tcp_connect_config(v: &Yaml) -> anyhow::Result<TcpConnectConfig> {
    if let Yaml::Hash(map) = v {
        let mut config = TcpConnectConfig::default();

        crate::foreach_kv(map, |k, v| match crate::key::normalize(k).as_str() {
            "max_retry" => {
                let max_retry = crate::value::as_usize(v)?;
                config.set_max_retry(max_retry);
                Ok(())
            }
            "each_timeout" => {
                let each_timeout = crate::humanize::as_duration(v)?;
                config.set_each_timeout(each_timeout);
                Ok(())
            }
            _ => Err(anyhow!("invalid key {k}")),
        })?;

        Ok(config)
    } else {
        Err(anyhow!(
            "yaml value type for 'TcpConnectConfig' should be 'map'"
        ))
    }
}

pub fn as_happy_eyeballs_config(v: &Yaml) -> anyhow::Result<HappyEyeballsConfig> {
    if let Yaml::Hash(map) = v {
        let mut config = HappyEyeballsConfig::default();

        crate::foreach_kv(map, |k, v| match crate::key::normalize(k).as_str() {
            "resolution_delay" | "first_resolution_delay" => {
                let delay = crate::humanize::as_duration(v)?;
                config.set_resolution_delay(delay);
                Ok(())
            }
            "second_resolution_timeout" => {
                let timeout = crate::humanize::as_duration(v)?;
                config.set_second_resolution_timeout(timeout);
                Ok(())
            }
            "first_address_family_count" => {
                let count = crate::value::as_usize(v)?;
                config.set_first_address_family_count(count);
                Ok(())
            }
            "connection_attempt_delay" => {
                let delay = crate::humanize::as_duration(v)?;
                config.set_connection_attempt_delay(delay);
                Ok(())
            }
            _ => Err(anyhow!("invalid key {k}")),
        })?;

        Ok(config)
    } else {
        Err(anyhow!(
            "yaml value type for 'HappyEyeballsConfig' should be 'map'"
        ))
    }
}

pub fn as_tcp_keepalive_config(v: &Yaml) -> anyhow::Result<TcpKeepAliveConfig> {
    let mut config = TcpKeepAliveConfig::default();

    match v {
        Yaml::Hash(map) => {
            crate::foreach_kv(map, |k, v| match crate::key::normalize(k).as_str() {
                "enable" => {
                    let enable = crate::value::as_bool(v)?;
                    config.set_enable(enable);
                    Ok(())
                }
                "idle_time" => {
                    let idle_time = crate::humanize::as_duration(v)?;
                    config.set_idle_time(idle_time);
                    Ok(())
                }
                "probe_interval" => {
                    let interval = crate::humanize::as_duration(v)?;
                    config.set_probe_interval(interval);
                    Ok(())
                }
                "probe_count" => {
                    let count = crate::value::as_u32(v)?;
                    config.set_probe_count(count);
                    Ok(())
                }
                _ => Err(anyhow!("invalid key {k}")),
            })?;
        }
        Yaml::Boolean(enable) => {
            config.set_enable(*enable);
        }
        _ => {
            let idle_time =
                crate::humanize::as_duration(v).context("invalid tcp keepalive idle_time value")?;
            config.set_enable(true);
            config.set_idle_time(idle_time);
        }
    }

    Ok(config)
}

pub fn as_tcp_misc_sock_opts(v: &Yaml) -> anyhow::Result<TcpMiscSockOpts> {
    let mut config = TcpMiscSockOpts::default();

    if let Yaml::Hash(map) = v {
        crate::foreach_kv(map, |k, v| match crate::key::normalize(k).as_str() {
            "no_delay" => {
                let no_delay =
                    crate::value::as_bool(v).context(format!("invalid bool value for key {k}"))?;
                config.no_delay = Some(no_delay);
                Ok(())
            }
            "max_segment_size" | "mss" => {
                let mss =
                    crate::value::as_u32(v).context(format!("invalid u32 value for key {k}"))?;
                config.max_segment_size = Some(mss);
                Ok(())
            }
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
            "yaml value type for 'TcpMiscSockOpts' should be 'map'"
        ))
    }
}
