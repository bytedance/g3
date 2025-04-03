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

use std::path::Path;
use std::sync::Arc;
use std::time::Duration;

use anyhow::{Context, anyhow};
use ascii::AsciiString;
use slog::Logger;
use yaml_rust::{Yaml, yaml};

use g3_histogram::HistogramMetricsConfig;
use g3_types::metrics::{NodeName, StaticMetricsTags};
use g3_types::net::TcpListenConfig;
use g3_yaml::{HybridParser, YamlDocPosition};

mod registry;
pub(crate) use registry::{clear, get_all};

#[derive(Clone)]
pub(crate) struct KeyServerConfig {
    name: NodeName,
    #[allow(unused)]
    position: Option<YamlDocPosition>,
    pub(crate) shared_logger: Option<AsciiString>,
    pub(crate) listen: TcpListenConfig,
    pub(crate) multiplex_queue_depth: usize,
    pub(crate) request_read_timeout: Duration,
    pub(crate) duration_stats: HistogramMetricsConfig,
    #[cfg(feature = "openssl-async-job")]
    pub(crate) async_op_timeout: Duration,
    pub(crate) concurrency_limit: usize,
    pub(crate) extra_metrics_tags: Option<Arc<StaticMetricsTags>>,
}

impl KeyServerConfig {
    fn new(position: Option<YamlDocPosition>) -> Self {
        KeyServerConfig {
            name: NodeName::default(),
            position,
            shared_logger: None,
            listen: TcpListenConfig::default(),
            multiplex_queue_depth: 0,
            request_read_timeout: Duration::from_millis(100),
            duration_stats: HistogramMetricsConfig::default(),
            #[cfg(feature = "openssl-async-job")]
            async_op_timeout: Duration::from_secs(1),
            concurrency_limit: 0,
            extra_metrics_tags: None,
        }
    }

    #[inline]
    pub(crate) fn name(&self) -> &NodeName {
        &self.name
    }

    fn parse(map: &yaml::Hash, position: Option<YamlDocPosition>) -> anyhow::Result<Self> {
        let mut server = KeyServerConfig::new(position);

        g3_yaml::foreach_kv(map, |k, v| server.set(k, v))?;

        server.check()?;
        Ok(server)
    }

    fn check(&mut self) -> anyhow::Result<()> {
        if self.name.is_empty() {
            return Err(anyhow!("name is not set"));
        }
        self.listen.check().context("invalid listen address")?;
        Ok(())
    }

    fn set(&mut self, k: &str, v: &Yaml) -> anyhow::Result<()> {
        match g3_yaml::key::normalize(k).as_str() {
            "name" => {
                self.name = g3_yaml::value::as_metric_node_name(v)?;
                Ok(())
            }
            "shared_logger" => {
                let name = g3_yaml::value::as_ascii(v)?;
                self.shared_logger = Some(name);
                Ok(())
            }
            "extra_metrics_tags" => {
                let tags = g3_yaml::value::as_static_metrics_tags(v)
                    .context(format!("invalid static metrics tags value for key {k}"))?;
                self.extra_metrics_tags = Some(Arc::new(tags));
                Ok(())
            }
            "listen" => {
                self.listen = g3_yaml::value::as_tcp_listen_config(v)
                    .context(format!("invalid tcp listen config value for key {k}"))?;
                Ok(())
            }
            "multiplex_queue_depth" => {
                self.multiplex_queue_depth = g3_yaml::value::as_usize(v)?;
                Ok(())
            }
            "request_read_timeout" => {
                self.request_read_timeout = g3_yaml::humanize::as_duration(v)?;
                Ok(())
            }
            "duration_stats" | "duration_metrics" => {
                self.duration_stats = g3_yaml::value::as_histogram_metrics_config(v).context(
                    format!("invalid histogram metrics config value for key {k}"),
                )?;
                Ok(())
            }
            #[cfg(feature = "openssl-async-job")]
            "async_op_timeout" => {
                self.async_op_timeout = g3_yaml::humanize::as_duration(v)?;
                Ok(())
            }
            "concurrency_limit" => {
                self.concurrency_limit = g3_yaml::value::as_usize(v)?;
                Ok(())
            }
            _ => Err(anyhow!("invalid key {k}")),
        }
    }

    pub(crate) fn get_task_logger(&self) -> Logger {
        if let Some(shared_logger) = &self.shared_logger {
            crate::log::task::get_shared_logger(shared_logger.as_str(), self.name())
        } else {
            crate::log::task::get_logger(self.name())
        }
    }

    pub(crate) fn get_request_logger(&self) -> Logger {
        if let Some(shared_logger) = &self.shared_logger {
            crate::log::request::get_shared_logger(shared_logger.as_str(), self.name())
        } else {
            crate::log::request::get_logger(self.name())
        }
    }
}

pub(crate) fn load_all(v: &Yaml, conf_dir: &Path) -> anyhow::Result<()> {
    let parser = HybridParser::new(conf_dir, g3_daemon::opts::config_file_extension());
    parser.foreach_map(v, |map, position| {
        let server = KeyServerConfig::parse(map, position)?;
        registry::add(server, false)?;
        Ok(())
    })?;
    Ok(())
}
