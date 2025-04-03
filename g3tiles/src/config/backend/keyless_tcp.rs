/*
 * Copyright 2024 ByteDance and/or its affiliates.
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

use std::sync::Arc;
use std::time::Duration;

use anyhow::{Context, anyhow};
use rustls_pki_types::ServerName;
use yaml_rust::{Yaml, yaml};

use g3_histogram::HistogramMetricsConfig;
use g3_types::metrics::{NodeName, StaticMetricsTags};
use g3_types::net::{ConnectionPoolConfig, RustlsClientConfigBuilder, TcpKeepAliveConfig};
use g3_yaml::YamlDocPosition;

use super::{AnyBackendConfig, BackendConfig, BackendConfigDiffAction};
use crate::config::discover::DiscoverRegisterData;
use crate::module::keyless::MultiplexedUpstreamConnectionConfig;

const BACKEND_CONFIG_TYPE: &str = "KeylessTcp";

#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) struct KeylessTcpBackendConfig {
    name: NodeName,
    position: Option<YamlDocPosition>,
    pub(crate) discover: NodeName,
    pub(crate) discover_data: DiscoverRegisterData,
    pub(crate) extra_metrics_tags: Option<Arc<StaticMetricsTags>>,
    pub(crate) tls_client: Option<RustlsClientConfigBuilder>,
    pub(crate) tls_name: Option<ServerName<'static>>,
    pub(crate) duration_stats: HistogramMetricsConfig,

    pub(crate) request_buffer_size: usize,
    pub(crate) connection_config: MultiplexedUpstreamConnectionConfig,
    pub(crate) graceful_close_wait: Duration,
    pub(crate) connection_pool: ConnectionPoolConfig,
    pub(crate) tcp_keepalive: TcpKeepAliveConfig,
    pub(crate) wait_new_channel: bool,
}

impl KeylessTcpBackendConfig {
    fn new(position: Option<YamlDocPosition>) -> Self {
        KeylessTcpBackendConfig {
            name: NodeName::default(),
            position,
            discover: NodeName::default(),
            discover_data: DiscoverRegisterData::Null,
            extra_metrics_tags: None,
            tls_client: None,
            tls_name: None,
            duration_stats: HistogramMetricsConfig::default(),
            request_buffer_size: 128,
            connection_config: Default::default(),
            graceful_close_wait: Duration::from_secs(10),
            connection_pool: ConnectionPoolConfig::new(8192, 256),
            tcp_keepalive: TcpKeepAliveConfig::default(),
            wait_new_channel: false,
        }
    }

    pub(super) fn parse(
        map: &yaml::Hash,
        position: Option<YamlDocPosition>,
    ) -> anyhow::Result<Self> {
        let mut site = KeylessTcpBackendConfig::new(position);
        g3_yaml::foreach_kv(map, |k, v| site.set(k, v))?;
        site.check()?;
        Ok(site)
    }

    fn check(&mut self) -> anyhow::Result<()> {
        if self.name.is_empty() {
            return Err(anyhow!("name is not set"));
        }
        if self.discover.is_empty() {
            return Err(anyhow!("no discover set"));
        }
        if matches!(self.discover_data, DiscoverRegisterData::Null) {
            return Err(anyhow!("no discover data set"));
        }
        Ok(())
    }

    fn set(&mut self, k: &str, v: &Yaml) -> anyhow::Result<()> {
        match k {
            super::CONFIG_KEY_BACKEND_TYPE => Ok(()),
            super::CONFIG_KEY_BACKEND_NAME => {
                self.name = g3_yaml::value::as_metric_node_name(v)?;
                Ok(())
            }
            "discover" => {
                self.discover = g3_yaml::value::as_metric_node_name(v)?;
                Ok(())
            }
            "discover_data" => {
                self.discover_data = DiscoverRegisterData::Yaml(v.clone());
                Ok(())
            }
            "extra_metrics_tags" => {
                let tags = g3_yaml::value::as_static_metrics_tags(v)
                    .context(format!("invalid static metrics tags value for key {k}"))?;
                self.extra_metrics_tags = Some(Arc::new(tags));
                Ok(())
            }
            "tls_client" => {
                let lookup_dir = g3_daemon::config::get_lookup_dir(self.position.as_ref())?;
                let tls_client =
                    g3_yaml::value::as_rustls_client_config_builder(v, Some(lookup_dir))?;
                self.tls_client = Some(tls_client);
                Ok(())
            }
            "tls_name" => {
                let name = g3_yaml::value::as_rustls_server_name(v)?;
                self.tls_name = Some(name);
                Ok(())
            }
            "duration_stats" | "duration_metrics" => {
                self.duration_stats = g3_yaml::value::as_histogram_metrics_config(v).context(
                    format!("invalid histogram metrics config value for key {k}"),
                )?;
                Ok(())
            }
            "request_buffer_size" => {
                self.request_buffer_size = g3_yaml::value::as_usize(v)?;
                Ok(())
            }
            "response_recv_timeout" | "response_timeout" => {
                self.connection_config.response_timeout = g3_yaml::humanize::as_duration(v)
                    .context(format!("invalid humanize duration value for key {k}"))?;
                Ok(())
            }
            "connection_max_request_count" => {
                self.connection_config.max_request_count = g3_yaml::value::as_usize(v)?;
                Ok(())
            }
            "connection_alive_time" => {
                self.connection_config.max_alive_time = g3_yaml::humanize::as_duration(v)
                    .context(format!("invalid humanize duration value for key {k}"))?;
                Ok(())
            }
            "graceful_close_wait" => {
                self.graceful_close_wait = g3_yaml::humanize::as_duration(v)
                    .context(format!("invalid humanize duration value for key {k}"))?;
                Ok(())
            }
            "connection_pool" | "pool" => {
                self.connection_pool = g3_yaml::value::as_connection_pool_config(v)
                    .context(format!("invalid connection pool config value for {k}"))?;
                Ok(())
            }
            "tcp_keepalive" => {
                self.tcp_keepalive = g3_yaml::value::as_tcp_keepalive_config(v)?;
                Ok(())
            }
            "wait_new_channel" => {
                self.wait_new_channel = g3_yaml::value::as_bool(v)?;
                Ok(())
            }
            _ => Err(anyhow!("invalid key {k}")),
        }
    }
}

impl BackendConfig for KeylessTcpBackendConfig {
    fn name(&self) -> &NodeName {
        &self.name
    }

    fn position(&self) -> Option<YamlDocPosition> {
        self.position.clone()
    }

    fn backend_type(&self) -> &'static str {
        BACKEND_CONFIG_TYPE
    }

    fn diff_action(&self, new: &AnyBackendConfig) -> BackendConfigDiffAction {
        let AnyBackendConfig::KeylessTcp(config) = new else {
            return BackendConfigDiffAction::SpawnNew;
        };

        if self.eq(config) {
            return BackendConfigDiffAction::NoAction;
        }

        BackendConfigDiffAction::Reload
    }
}
