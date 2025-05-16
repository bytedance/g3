/*
 * SPDX-License-Identifier: Apache-2.0
 * Copyright 2023-2025 ByteDance and/or its affiliates.
 */

use std::path::Path;
use std::sync::Arc;
use std::time::Duration;

use anyhow::{Context, anyhow};
use ascii::AsciiString;
use slog::Logger;
use yaml_rust::{Yaml, yaml};

use g3_histogram::HistogramMetricsConfig;
use g3_types::metrics::{MetricTagMap, NodeName};
use g3_types::net::{OpensslServerConfigBuilder, TcpListenConfig};
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
    pub(crate) tls_server: Option<OpensslServerConfigBuilder>,
    pub(crate) multiplex_queue_depth: usize,
    pub(crate) request_read_timeout: Duration,
    pub(crate) duration_stats: HistogramMetricsConfig,
    #[cfg(feature = "openssl-async-job")]
    pub(crate) async_op_timeout: Duration,
    pub(crate) concurrency_limit: usize,
    pub(crate) extra_metrics_tags: Option<Arc<MetricTagMap>>,
}

impl KeyServerConfig {
    fn new(position: Option<YamlDocPosition>) -> Self {
        KeyServerConfig {
            name: NodeName::default(),
            position,
            shared_logger: None,
            listen: TcpListenConfig::default(),
            tls_server: None,
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
            "tls" | "tls_server" => {
                let lookup_dir = g3_daemon::config::get_lookup_dir(self.position.as_ref())?;
                let tls_server =
                    g3_yaml::value::as_openssl_tls_server_config_builder(v, Some(lookup_dir))
                        .context(format!("invalid server tls config value for key {k}"))?;
                self.tls_server = Some(tls_server);
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

    pub(crate) fn get_task_logger(&self) -> Option<Logger> {
        if let Some(shared_logger) = &self.shared_logger {
            crate::log::task::get_shared_logger(shared_logger.as_str(), self.name())
        } else {
            crate::log::task::get_logger(self.name())
        }
    }

    pub(crate) fn get_request_logger(&self) -> Option<Logger> {
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
