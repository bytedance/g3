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

use anyhow::{anyhow, Context};
use serde_json::Value;

use g3_types::net::{TcpConnectConfig, TcpKeepAliveConfig, TcpMiscSockOpts};

pub fn as_tcp_connect_config(v: &Value) -> anyhow::Result<TcpConnectConfig> {
    if let Value::Object(map) = v {
        let mut config = TcpConnectConfig::default();

        for (k, v) in map {
            match crate::key::normalize(k).as_str() {
                "max_retry" => {
                    let max_retry = crate::value::as_usize(v)
                        .context(format!("invalid usize value for key {k}"))?;
                    config.set_max_retry(max_retry);
                }
                "each_timeout" => {
                    let each_timeout = crate::humanize::as_duration(v)
                        .context(format!("invalid humanize duration value for key {k}"))?;
                    config.set_each_timeout(each_timeout);
                }
                _ => return Err(anyhow!("invalid key {k}")),
            }
        }

        Ok(config)
    } else {
        Err(anyhow!(
            "json value type for 'TcpConnectConfig' should be 'map'"
        ))
    }
}

pub fn as_tcp_keepalive_config(v: &Value) -> anyhow::Result<TcpKeepAliveConfig> {
    let mut config = TcpKeepAliveConfig::default();

    match v {
        Value::Object(map) => {
            for (k, v) in map {
                match crate::key::normalize(k).as_str() {
                    "enable" => {
                        let enable = crate::value::as_bool(v)
                            .context(format!("invalid boolean value for key {k}"))?;
                        config.set_enable(enable);
                    }
                    "idle_time" => {
                        let idle_time = crate::humanize::as_duration(v)
                            .context(format!("invalid humanize duration value for key {k}"))?;
                        config.set_idle_time(idle_time);
                    }
                    "probe_interval" => {
                        let interval = crate::humanize::as_duration(v)
                            .context(format!("invalid humanize duration value for key {k}"))?;
                        config.set_probe_interval(interval);
                    }
                    "probe_count" => {
                        let count = crate::value::as_u32(v)
                            .context(format!("invalid u32 value for key {k}"))?;
                        config.set_probe_count(count);
                    }
                    _ => return Err(anyhow!("invalid key {k}")),
                }
            }
        }
        Value::Bool(enable) => {
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

pub fn as_tcp_misc_sock_opts(v: &Value) -> anyhow::Result<TcpMiscSockOpts> {
    let mut config = TcpMiscSockOpts::default();

    if let Value::Object(map) = v {
        for (k, v) in map {
            match crate::key::normalize(k).as_str() {
                "no_delay" => {
                    let no_delay = crate::value::as_bool(v)
                        .context(format!("invalid bool value for key {k}"))?;
                    config.no_delay = Some(no_delay);
                }
                "max_segment_size" | "mss" => {
                    let mss = crate::value::as_u32(v)
                        .context(format!("invalid u32 value for key {k}"))?;
                    config.max_segment_size = Some(mss);
                }
                "time_to_live" | "ttl" => {
                    let ttl = crate::value::as_u32(v)
                        .context(format!("invalid u32 value for key {k}"))?;
                    config.time_to_live = Some(ttl);
                }
                "type_of_service" | "tos" => {
                    let tos =
                        crate::value::as_u8(v).context(format!("invalid u8 value for key {k}"))?;
                    config.type_of_service = Some(tos);
                }
                "netfilter_mark" | "mark" => {
                    let mark = crate::value::as_u32(v)
                        .context(format!("invalid u32 value for key {k}"))?;
                    config.netfilter_mark = Some(mark);
                }
                _ => return Err(anyhow!("invalid key {k}")),
            }
        }

        Ok(config)
    } else {
        Err(anyhow!(
            "json value type for 'TcpMiscSockOpts' should be 'map'"
        ))
    }
}
