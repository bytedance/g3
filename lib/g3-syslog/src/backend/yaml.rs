/*
 * SPDX-License-Identifier: Apache-2.0
 * Copyright 2023-2025 ByteDance and/or its affiliates.
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
