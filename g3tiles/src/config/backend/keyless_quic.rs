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
use yaml_rust::{Yaml, yaml};

use g3_histogram::HistogramMetricsConfig;
use g3_types::metrics::{MetricTagMap, NodeName};
use g3_types::net::{
    ConnectionPoolConfig, QuinnTransportConfigBuilder, RustlsClientConfigBuilder,
    SocketBufferConfig,
};
use g3_yaml::YamlDocPosition;

const BACKEND_CONFIG_TYPE: &str = "KeylessQuic";

use super::{AnyBackendConfig, BackendConfig, BackendConfigDiffAction};
use crate::config::discover::DiscoverRegisterData;
use crate::module::keyless::MultiplexedUpstreamConnectionConfig;

#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) struct KeylessQuicBackendConfig {
    name: NodeName,
    position: Option<YamlDocPosition>,
    pub(crate) discover: NodeName,
    pub(crate) discover_data: DiscoverRegisterData,
    pub(crate) extra_metrics_tags: Option<Arc<MetricTagMap>>,
    pub(crate) tls_client: RustlsClientConfigBuilder,
    pub(crate) tls_name: Option<String>,
    pub(crate) duration_stats: HistogramMetricsConfig,

    pub(crate) request_buffer_size: usize,
    pub(crate) connection_config: MultiplexedUpstreamConnectionConfig,
    pub(crate) graceful_close_wait: Duration,
    pub(crate) connection_pool: ConnectionPoolConfig,
    pub(crate) quic_transport: QuinnTransportConfigBuilder,
    pub(crate) concurrent_streams: usize,
    pub(crate) wait_new_channel: bool,
    pub(crate) socket_buffer: SocketBufferConfig,
}

impl KeylessQuicBackendConfig {
    fn new(position: Option<YamlDocPosition>) -> Self {
        KeylessQuicBackendConfig {
            name: NodeName::default(),
            position,
            discover: NodeName::default(),
            discover_data: DiscoverRegisterData::Null,
            extra_metrics_tags: None,
            tls_client: RustlsClientConfigBuilder::default(),
            tls_name: None,
            duration_stats: HistogramMetricsConfig::default(),
            request_buffer_size: 128,
            connection_config: Default::default(),
            graceful_close_wait: Duration::from_secs(10),
            connection_pool: ConnectionPoolConfig::new(2048, 128),
            quic_transport: QuinnTransportConfigBuilder::default(),
            concurrent_streams: 4,
            wait_new_channel: false,
            socket_buffer: SocketBufferConfig::default(),
        }
    }

    pub(super) fn parse(
        map: &yaml::Hash,
        position: Option<YamlDocPosition>,
    ) -> anyhow::Result<Self> {
        let mut site = KeylessQuicBackendConfig::new(position);
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
        if self.concurrent_streams == 0 {
            self.concurrent_streams = 1;
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
                self.tls_client =
                    g3_yaml::value::as_rustls_client_config_builder(v, Some(lookup_dir))?;
                Ok(())
            }
            "tls_name" => {
                let name = g3_yaml::value::as_string(v)?;
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
                    .context(format!("invalid connection pool config value for key {k}"))?;
                Ok(())
            }
            "quic_transport" => {
                self.quic_transport = g3_yaml::value::as_quinn_transport_config(v)
                    .context(format!("invalid quinn transport config value for key {k}"))?;
                Ok(())
            }
            "concurrent_streams" => {
                self.concurrent_streams = g3_yaml::value::as_usize(v)?;
                Ok(())
            }
            "wait_new_channel" => {
                self.wait_new_channel = g3_yaml::value::as_bool(v)?;
                Ok(())
            }
            "socket_buffer" => {
                self.socket_buffer = g3_yaml::value::as_socket_buffer_config(v)?;
                Ok(())
            }
            _ => Err(anyhow!("invalid key {k}")),
        }
    }
}

impl BackendConfig for KeylessQuicBackendConfig {
    fn name(&self) -> &NodeName {
        &self.name
    }

    fn position(&self) -> Option<YamlDocPosition> {
        self.position.clone()
    }

    fn r#type(&self) -> &'static str {
        BACKEND_CONFIG_TYPE
    }

    fn diff_action(&self, new: &AnyBackendConfig) -> BackendConfigDiffAction {
        let AnyBackendConfig::KeylessQuic(config) = new else {
            return BackendConfigDiffAction::SpawnNew;
        };

        if self.eq(config) {
            return BackendConfigDiffAction::NoAction;
        }

        BackendConfigDiffAction::Reload
    }
}
