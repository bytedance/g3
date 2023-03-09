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

use std::net::{IpAddr, SocketAddr};
use std::path::PathBuf;
use std::str::FromStr;

use anyhow::{anyhow, Context};
use yaml_rust::Yaml;

use g3_statsd::client::{StatsdBackend, StatsdClientConfig};
use g3_types::metrics::MetricsName;

fn as_statsd_backend_udp(v: &Yaml) -> anyhow::Result<StatsdBackend> {
    match v {
        Yaml::Hash(map) => {
            let mut addr: Option<SocketAddr> = None;
            let mut bind: Option<IpAddr> = None;

            crate::foreach_kv(map, |k, v| match crate::key::normalize(k).as_str() {
                "address" | "addr" => {
                    addr = Some(
                        crate::value::as_sockaddr(v)
                            .context(format!("invalid value for key {k}"))?,
                    );
                    Ok(())
                }
                "bind_ip" | "bind" => {
                    bind = Some(
                        crate::value::as_ipaddr(v).context(format!("invalid value for key {k}"))?,
                    );
                    Ok(())
                }
                _ => Err(anyhow!("invalid key {k}")),
            })?;

            if let Some(addr) = addr.take() {
                Ok(StatsdBackend::Udp(addr, bind))
            } else {
                Err(anyhow!("no target address has been set"))
            }
        }
        Yaml::String(s) => {
            let addr = SocketAddr::from_str(s).map_err(|e| anyhow!("invalid SocketAddr: {e}"))?;
            Ok(StatsdBackend::Udp(addr, None))
        }
        _ => Err(anyhow!("invalid yaml value for udp statsd backend")),
    }
}

fn as_statsd_backend_unix(v: &Yaml) -> anyhow::Result<StatsdBackend> {
    match v {
        Yaml::Hash(map) => {
            let mut path: Option<PathBuf> = None;

            crate::foreach_kv(map, |k, v| match crate::key::normalize(k).as_str() {
                "path" => {
                    path = Some(
                        crate::value::as_absolute_path(v)
                            .context(format!("invalid value for key {k}"))?,
                    );
                    Ok(())
                }
                _ => Err(anyhow!("invalid key {k}")),
            })?;
            if let Some(path) = path.take() {
                Ok(StatsdBackend::Unix(path))
            } else {
                Err(anyhow!("no path has been set"))
            }
        }
        Yaml::String(_) => {
            let path = crate::value::as_absolute_path(v)?;
            Ok(StatsdBackend::Unix(path))
        }
        _ => Err(anyhow!("invalid yaml value for unix statsd backend")),
    }
}

pub fn as_statsd_client_config(
    v: &Yaml,
    prefix: MetricsName,
) -> anyhow::Result<StatsdClientConfig> {
    if let Yaml::Hash(map) = v {
        let mut config = StatsdClientConfig::with_prefix(prefix);

        crate::hash::foreach_kv(map, |k, v| match crate::key::normalize(k).as_str() {
            "target_udp" | "backend_udp" => {
                let target =
                    as_statsd_backend_udp(v).context(format!("invalid value for key {k}"))?;
                config.set_backend(target);
                Ok(())
            }
            "target_unix" | "backend_unix" => {
                let target =
                    as_statsd_backend_unix(v).context(format!("invalid value for key {k}"))?;
                config.set_backend(target);
                Ok(())
            }
            "target" | "backend" => {
                if let Yaml::Hash(map) = v {
                    crate::hash::foreach_kv(map, |k, v| match crate::key::normalize(k).as_str() {
                        "udp" => {
                            let target = as_statsd_backend_udp(v)
                                .context(format!("invalid value for key {k}"))?;
                            config.set_backend(target);
                            Ok(())
                        }
                        "unix" => {
                            let target = as_statsd_backend_unix(v)
                                .context(format!("invalid value for key {k}"))?;
                            config.set_backend(target);
                            Ok(())
                        }
                        _ => Err(anyhow!("invalid key {k}")),
                    })
                    .context(format!("invalid value for key {k}"))
                } else {
                    Err(anyhow!("yaml value type for key {k} should be 'map'"))
                }
            }
            "prefix" => {
                let prefix = crate::value::as_metrics_name(v)
                    .context(format!("invalid metrics name value for key {k}"))?;
                config.set_prefix(prefix);
                Ok(())
            }
            "emit_duration" => {
                config.emit_duration = crate::humanize::as_duration(v)
                    .context(format!("invalid humanize duration value for key {k}"))?;
                Ok(())
            }
            _ => Err(anyhow!("invalid key {k}")),
        })?;

        Ok(config)
    } else {
        Err(anyhow!(
            "yaml value type for 'statsd client config' should be 'map'"
        ))
    }
}
