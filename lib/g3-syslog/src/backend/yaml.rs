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
#[cfg(unix)]
use std::path::PathBuf;
use std::str::FromStr;

use anyhow::{Context, anyhow};
use yaml_rust::Yaml;

use super::SyslogBackendBuilder;

impl SyslogBackendBuilder {
    pub(crate) fn parse_udp_yaml(value: &Yaml) -> anyhow::Result<Self> {
        match value {
            Yaml::Hash(map) => {
                let mut addr: Option<SocketAddr> = None;
                let mut bind: Option<IpAddr> = None;

                g3_yaml::foreach_kv(map, |k, v| match g3_yaml::key::normalize(k).as_str() {
                    "address" | "addr" => {
                        addr = Some(g3_yaml::value::as_env_sockaddr(v).context(format!(
                            "invalid syslog udp peer socket address value for key {k}"
                        ))?);
                        Ok(())
                    }
                    "bind_ip" | "bind" => {
                        bind = Some(
                            g3_yaml::value::as_ipaddr(v)
                                .context(format!("invalid value for key {k}"))?,
                        );
                        Ok(())
                    }
                    _ => Err(anyhow!("invalid key {k}")),
                })?;

                if let Some(addr) = addr.take() {
                    Ok(SyslogBackendBuilder::Udp(bind, addr))
                } else {
                    Err(anyhow!("no target address has been set"))
                }
            }
            Yaml::String(s) => {
                let addr =
                    SocketAddr::from_str(s).map_err(|e| anyhow!("invalid SocketAddr: {e}"))?;
                Ok(SyslogBackendBuilder::Udp(None, addr))
            }
            _ => Err(anyhow!("invalid yaml value for udp syslog backend")),
        }
    }

    #[cfg(unix)]
    pub(crate) fn parse_unix_yaml(value: &Yaml) -> anyhow::Result<Self> {
        match value {
            Yaml::Hash(map) => {
                let mut path: Option<PathBuf> = None;

                g3_yaml::foreach_kv(map, |k, v| match g3_yaml::key::normalize(k).as_str() {
                    "path" => {
                        path = Some(
                            g3_yaml::value::as_absolute_path(v)
                                .context(format!("invalid value for key {k}"))?,
                        );
                        Ok(())
                    }
                    _ => Err(anyhow!("invalid key {k}")),
                })?;
                if let Some(path) = path.take() {
                    Ok(SyslogBackendBuilder::Unix(Some(path)))
                } else {
                    Err(anyhow!("no path has been set"))
                }
            }
            Yaml::String(_) => {
                let path = g3_yaml::value::as_absolute_path(value)?;
                Ok(SyslogBackendBuilder::Unix(Some(path)))
            }
            _ => Err(anyhow!("invalid yaml value for unix syslog backend")),
        }
    }
}
