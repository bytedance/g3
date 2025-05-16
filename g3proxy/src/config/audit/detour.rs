/*
 * SPDX-License-Identifier: Apache-2.0
 * Copyright 2024-2025 ByteDance and/or its affiliates.
 */

use std::net::{IpAddr, Ipv4Addr};
use std::num::NonZeroUsize;
use std::str::FromStr;
use std::time::Duration;

use anyhow::{Context, anyhow};
use yaml_rust::Yaml;

use g3_types::net::{
    ConnectionPoolConfig, QuinnTransportConfigBuilder, RustlsClientConfigBuilder,
    SocketBufferConfig, UpstreamAddr,
};
use g3_yaml::YamlDocPosition;

const DEFAULT_DETOUR_PORT: u16 = 2888;

pub(crate) struct AuditStreamDetourConfig {
    pub(crate) peer_addr: UpstreamAddr,
    pub(crate) tls_client: RustlsClientConfigBuilder,
    pub(crate) tls_name: Option<String>,
    pub(crate) connection_pool: ConnectionPoolConfig,
    pub(crate) connection_reuse_limit: NonZeroUsize,
    pub(crate) quic_transport: QuinnTransportConfigBuilder,
    pub(crate) stream_open_timeout: Duration,
    pub(crate) request_timeout: Duration,
    pub(crate) socket_buffer: SocketBufferConfig,
}

impl Default for AuditStreamDetourConfig {
    fn default() -> Self {
        AuditStreamDetourConfig {
            peer_addr: UpstreamAddr::from_ip_and_port(
                IpAddr::V4(Ipv4Addr::LOCALHOST),
                DEFAULT_DETOUR_PORT,
            ),
            tls_client: RustlsClientConfigBuilder::default(),
            tls_name: None,
            connection_pool: ConnectionPoolConfig::default(),
            connection_reuse_limit: NonZeroUsize::new(16).unwrap(),
            quic_transport: QuinnTransportConfigBuilder::default(),
            stream_open_timeout: Duration::from_secs(30),
            request_timeout: Duration::from_secs(60),
            socket_buffer: SocketBufferConfig::default(),
        }
    }
}

impl AuditStreamDetourConfig {
    pub(super) fn parse(value: &Yaml, position: Option<&YamlDocPosition>) -> anyhow::Result<Self> {
        let mut config = AuditStreamDetourConfig::default();

        match value {
            Yaml::Hash(map) => {
                g3_yaml::foreach_kv(map, |k, v| match g3_yaml::key::normalize(k).as_str() {
                    "peer" | "peer_addr" => {
                        config.peer_addr = g3_yaml::value::as_upstream_addr(v, DEFAULT_DETOUR_PORT)
                            .context(format!("invalid upstream address value for key {k}"))?;
                        Ok(())
                    }
                    "tls_client" => {
                        let lookup_dir = g3_daemon::config::get_lookup_dir(position)?;
                        config.tls_client =
                            g3_yaml::value::as_rustls_client_config_builder(v, Some(lookup_dir))
                                .context(format!(
                                    "invalid rustls tls client config value for key {k}"
                                ))?;
                        Ok(())
                    }
                    "tls_name" => {
                        let name = g3_yaml::value::as_string(v)?;
                        config.tls_name = Some(name);
                        Ok(())
                    }
                    "connection_pool" | "pool" => {
                        config.connection_pool = g3_yaml::value::as_connection_pool_config(v)
                            .context(format!("invalid connection pool config value for key {k}"))?;
                        Ok(())
                    }
                    "connection_reuse_limit" => {
                        config.connection_reuse_limit = g3_yaml::value::as_nonzero_usize(v)?;
                        Ok(())
                    }
                    "quic_transport" => {
                        config.quic_transport = g3_yaml::value::as_quinn_transport_config(v)
                            .context(format!("invalid quinn transport config value for key {k}"))?;
                        Ok(())
                    }
                    "stream_open_timeout" => {
                        config.stream_open_timeout = g3_yaml::humanize::as_duration(v)
                            .context(format!("invalid humanize duration value for key {k}"))?;
                        Ok(())
                    }
                    "request_timeout" => {
                        config.request_timeout = g3_yaml::humanize::as_duration(v)
                            .context(format!("invalid humanize duration value for key {k}"))?;
                        Ok(())
                    }
                    "socket_buffer" => {
                        config.socket_buffer = g3_yaml::value::as_socket_buffer_config(v)?;
                        Ok(())
                    }
                    _ => Err(anyhow!("invalid key {k}")),
                })?;
            }
            Yaml::String(s) => {
                config.peer_addr =
                    UpstreamAddr::from_str(s).context("invalid upstream address string value")?;
                if config.peer_addr.port() == 0 {
                    config.peer_addr.set_port(DEFAULT_DETOUR_PORT);
                }
            }
            Yaml::Integer(i) => {
                let port =
                    u16::try_from(*i).map_err(|e| anyhow!("out of range u16 port value: {e}"))?;
                config.peer_addr.set_port(port);
            }
            _ => {
                return Err(anyhow!(
                    "invalid yaml value type for audit stream detour config"
                ));
            }
        }

        Ok(config)
    }
}
