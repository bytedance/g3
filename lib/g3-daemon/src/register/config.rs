/*
 * SPDX-License-Identifier: Apache-2.0
 * Copyright 2023-2025 ByteDance and/or its affiliates.
 */

use std::time::Duration;

use anyhow::{Context, anyhow};
use http::uri::PathAndQuery;
use yaml_rust::{Yaml, yaml};

use g3_types::metrics::MetricTagMap;
use g3_types::net::UpstreamAddr;

pub struct RegisterConfig {
    pub(crate) upstream: UpstreamAddr,
    pub(crate) startup_retry: usize,
    pub(crate) retry_interval: Duration,
    pub(crate) register_path: PathAndQuery,
    pub(crate) ping_path: PathAndQuery,
    pub(crate) ping_interval: Duration,
    pub(crate) extra_data: MetricTagMap,
}

impl Default for RegisterConfig {
    fn default() -> Self {
        RegisterConfig {
            upstream: UpstreamAddr::empty(),
            startup_retry: 3,
            retry_interval: Duration::from_secs(1),
            register_path: PathAndQuery::from_static("/register"),
            ping_path: PathAndQuery::from_static("/ping"),
            ping_interval: Duration::from_secs(60),
            extra_data: MetricTagMap::default(),
        }
    }
}

impl RegisterConfig {
    #[inline]
    pub fn startup_retry(&self) -> usize {
        self.startup_retry
    }

    #[inline]
    pub fn retry_interval(&self) -> Duration {
        self.retry_interval
    }

    pub(crate) fn parse(&mut self, v: &Yaml) -> anyhow::Result<()> {
        match v {
            Yaml::Hash(map) => self.parse_map(map),
            Yaml::String(_) => {
                self.upstream = g3_yaml::value::as_upstream_addr(v, 0)
                    .context("invalid upstream address string value")?;
                Ok(())
            }
            _ => Err(anyhow!("invalid yaml value type")),
        }
    }

    fn parse_map(&mut self, map: &yaml::Hash) -> anyhow::Result<()> {
        g3_yaml::foreach_kv(map, |k, v| match g3_yaml::key::normalize(k).as_str() {
            "upstream" => {
                self.upstream = g3_yaml::value::as_upstream_addr(v, 0)
                    .context(format!("invalid upstream address value for key {k}"))?;
                Ok(())
            }
            "startup_retry" => {
                self.startup_retry = g3_yaml::value::as_usize(v)?;
                Ok(())
            }
            "retry_interval" => {
                self.retry_interval = g3_yaml::humanize::as_duration(v)
                    .context(format!("invalid humanize duration value for key {k}"))?;
                Ok(())
            }
            "register_path" => {
                self.register_path = g3_yaml::value::as_http_path_and_query(v)
                    .context(format!("invalid http path_query value for key {k}"))?;
                Ok(())
            }
            "ping_path" => {
                self.ping_path = g3_yaml::value::as_http_path_and_query(v)
                    .context(format!("invalid http path_query value for key {k}"))?;
                Ok(())
            }
            "ping_interval" => {
                self.ping_interval = g3_yaml::humanize::as_duration(v)
                    .context(format!("invalid humanize duration value for key {k}"))?;
                Ok(())
            }
            "extra_data" => {
                self.extra_data = g3_yaml::value::as_static_metrics_tags(v)
                    .context(format!("invalid static metrics tags value for key {k}"))?;
                Ok(())
            }
            _ => Err(anyhow!("invalid key {k}")),
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use g3_yaml::{yaml_doc, yaml_str};
    use yaml_rust::YamlLoader;

    #[test]
    fn default() {
        let config = RegisterConfig::default();

        assert!(config.upstream.is_empty());
        assert_eq!(config.startup_retry(), 3);
        assert_eq!(config.retry_interval(), Duration::from_secs(1));
        assert_eq!(config.register_path.as_str(), "/register");
        assert_eq!(config.ping_path.as_str(), "/ping");
        assert_eq!(config.ping_interval, Duration::from_secs(60));
        assert!(config.extra_data.is_empty());
    }

    #[test]
    fn parse_string_upstream() {
        let mut config = RegisterConfig::default();

        // Ok case
        let yaml = yaml_str!("127.0.0.1:8080");
        assert!(config.parse(&yaml).is_ok());
        assert_eq!(config.upstream.to_string(), "127.0.0.1:8080");

        // Err case
        let yaml = yaml_str!("invalid-address");
        assert!(config.parse(&yaml).is_err());
    }

    #[test]
    fn parse_invalid_yaml_type() {
        let mut config = RegisterConfig::default();

        let yaml = Yaml::Array(vec![]);
        assert!(config.parse(&yaml).is_err());

        let yaml = Yaml::Integer(123);
        assert!(config.parse(&yaml).is_err());

        let yaml = Yaml::Boolean(true);
        assert!(config.parse(&yaml).is_err());
    }

    #[test]
    fn parse_map_ok() {
        let mut config = RegisterConfig::default();

        let yaml = yaml_doc!(
            r#"
                upstream: "example.com:443"
                startup_retry: "5"
                retry_interval: 10
                register_path: "/api/register?version=1"
                ping_path: "/health"
                ping_interval: "2m"
                extra_data:
                  service: "test-service"
                  version: "1.0"
            "#
        );
        assert!(config.parse(&yaml).is_ok());
        assert_eq!(config.upstream.to_string(), "example.com:443");
        assert_eq!(config.startup_retry(), 5);
        assert_eq!(config.retry_interval(), Duration::from_secs(10));
        assert_eq!(config.register_path.as_str(), "/api/register?version=1");
        assert_eq!(config.ping_path.as_str(), "/health");
        assert_eq!(config.ping_interval, Duration::from_secs(120));
        assert_eq!(config.extra_data.len(), 2);

        let yaml = yaml_doc!(
            r#"
                upstream: "10.0.0.1:8080"
                startup_retry: 7
                retry_interval: "3s"
                register_path: "/register"
                ping_path: "/ping"
                ping_interval: "45s"
                extra_data: {}
            "#
        );
        assert!(config.parse(&yaml).is_ok());
        assert_eq!(config.upstream.to_string(), "10.0.0.1:8080");
        assert_eq!(config.startup_retry(), 7);
        assert_eq!(config.retry_interval(), Duration::from_secs(3));
        assert_eq!(config.register_path.as_str(), "/register");
        assert_eq!(config.ping_path.as_str(), "/ping");
        assert_eq!(config.ping_interval, Duration::from_secs(45));
        assert!(config.extra_data.is_empty());
    }

    #[test]
    fn parse_map_err() {
        let mut config = RegisterConfig::default();

        let yaml = yaml_doc!(
            r#"
                upstream: 12345
            "#
        );
        assert!(config.parse(&yaml).is_err());

        let yaml = yaml_doc!(
            r#"
                startup_retry: -1
            "#
        );
        assert!(config.parse(&yaml).is_err());

        let yaml = yaml_doc!(
            r#"
                retry_interval: "invalid"
            "#
        );
        assert!(config.parse(&yaml).is_err());

        let yaml = yaml_doc!(
            r#"
                register_path: 123
            "#
        );
        assert!(config.parse(&yaml).is_err());

        let yaml = yaml_doc!(
            r#"
                ping_path: []
            "#
        );
        assert!(config.parse(&yaml).is_err());

        let yaml = yaml_doc!(
            r#"
                ping_interval: "not-a-duration"
            "#
        );
        assert!(config.parse(&yaml).is_err());

        let yaml = yaml_doc!(
            r#"
                extra_data: "not-a-map"
            "#
        );
        assert!(config.parse(&yaml).is_err());

        let yaml = yaml_doc!(
            r#"
                invalid_key: "value"
            "#
        );
        assert!(config.parse(&yaml).is_err());
    }

    #[test]
    fn parse_edge_cases() {
        // Zero startup retry and very small interval
        let mut config = RegisterConfig::default();
        let yaml = yaml_doc!(
            r#"
                startup_retry: 0
                retry_interval: "1ms"
            "#
        );
        assert!(config.parse(&yaml).is_ok());
        assert_eq!(config.startup_retry(), 0);
        assert_eq!(config.retry_interval(), Duration::from_millis(1));
    }
}
