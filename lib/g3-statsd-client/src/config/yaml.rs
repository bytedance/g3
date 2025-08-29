/*
 * SPDX-License-Identifier: Apache-2.0
 * Copyright 2023-2025 ByteDance and/or its affiliates.
 */

use std::net::{IpAddr, SocketAddr};
#[cfg(unix)]
use std::path::PathBuf;
use std::str::FromStr;

use anyhow::{Context, anyhow};
use log::warn;
use yaml_rust::Yaml;

use g3_types::metrics::NodeName;

use super::{StatsdBackend, StatsdClientConfig};

impl StatsdBackend {
    pub fn parse_udp_yaml(v: &Yaml) -> anyhow::Result<Self> {
        match v {
            Yaml::Hash(map) => {
                let mut addr: Option<SocketAddr> = None;
                let mut bind: Option<IpAddr> = None;

                g3_yaml::foreach_kv(map, |k, v| match g3_yaml::key::normalize(k).as_str() {
                    "address" | "addr" => {
                        addr = Some(g3_yaml::value::as_env_sockaddr(v).context(format!(
                            "invalid statsd udp peer socket address value for key {k}"
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
                    Ok(StatsdBackend::Udp(addr, bind))
                } else {
                    Err(anyhow!("no target address has been set"))
                }
            }
            Yaml::String(s) => {
                let addr =
                    SocketAddr::from_str(s).map_err(|e| anyhow!("invalid SocketAddr: {e}"))?;
                Ok(StatsdBackend::Udp(addr, None))
            }
            _ => Err(anyhow!("invalid yaml value for udp statsd backend")),
        }
    }

    #[cfg(unix)]
    pub fn parse_unix_yaml(v: &Yaml) -> anyhow::Result<Self> {
        match v {
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
                    Ok(StatsdBackend::Unix(path))
                } else {
                    Err(anyhow!("no path has been set"))
                }
            }
            Yaml::String(_) => {
                let path = g3_yaml::value::as_absolute_path(v)?;
                Ok(StatsdBackend::Unix(path))
            }
            _ => Err(anyhow!("invalid yaml value for unix statsd backend")),
        }
    }
}

impl StatsdClientConfig {
    pub fn parse_yaml(v: &Yaml, prefix: NodeName) -> anyhow::Result<Self> {
        if let Yaml::Hash(map) = v {
            let mut config = StatsdClientConfig::with_prefix(prefix);
            g3_yaml::foreach_kv(map, |k, v| config.set_by_yaml_kv(k, v))?;
            Ok(config)
        } else {
            Err(anyhow!(
                "yaml value type for 'statsd client config' should be 'map'"
            ))
        }
    }

    fn set_by_yaml_kv(&mut self, k: &str, v: &Yaml) -> anyhow::Result<()> {
        match g3_yaml::key::normalize(k).as_str() {
            "target_udp" | "backend_udp" => {
                let target = StatsdBackend::parse_udp_yaml(v)
                    .context(format!("invalid value for key {k}"))?;
                self.set_backend(target);
            }
            #[cfg(unix)]
            "target_unix" | "backend_unix" => {
                let target = StatsdBackend::parse_unix_yaml(v)
                    .context(format!("invalid value for key {k}"))?;
                self.set_backend(target);
            }
            "target" | "backend" => {
                return if let Yaml::Hash(map) = v {
                    g3_yaml::foreach_kv(map, |k, v| match g3_yaml::key::normalize(k).as_str() {
                        "udp" => {
                            let target = StatsdBackend::parse_udp_yaml(v)
                                .context(format!("invalid value for key {k}"))?;
                            self.set_backend(target);
                            Ok(())
                        }
                        #[cfg(unix)]
                        "unix" => {
                            let target = StatsdBackend::parse_unix_yaml(v)
                                .context(format!("invalid value for key {k}"))?;
                            self.set_backend(target);
                            Ok(())
                        }
                        _ => Err(anyhow!("invalid key {k}")),
                    })
                    .context(format!("invalid value for key {k}"))
                } else {
                    Err(anyhow!("yaml value type for key {k} should be 'map'"))
                };
            }
            "prefix" => {
                let prefix = g3_yaml::value::as_metric_node_name(v)
                    .context(format!("invalid metrics name value for key {k}"))?;
                self.set_prefix(prefix);
            }
            "cache_size" => {
                self.cache_size = g3_yaml::humanize::as_usize(v)
                    .context(format!("invalid humanize usize value for key {k}"))?;
            }
            "max_segment_size" => {
                let size = g3_yaml::humanize::as_usize(v)
                    .context(format!("invalid humanize usize value for key {k}"))?;
                self.max_segment_size = Some(size);
            }
            "emit_duration" => {
                warn!("deprecated config key '{k}', please use 'emit_interval' instead");
                return self.set_by_yaml_kv("emit_interval", v);
            }
            "emit_interval" => {
                self.emit_interval = g3_yaml::humanize::as_duration(v)
                    .context(format!("invalid humanize duration value for key {k}"))?;
            }
            _ => return Err(anyhow!("invalid key {k}")),
        }
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use g3_yaml::yaml_doc;
    use std::net::Ipv4Addr;
    use std::time::Duration;
    use yaml_rust::YamlLoader;

    fn default_node_name() -> NodeName {
        NodeName::from_str("test").unwrap()
    }

    #[test]
    fn parse_udp_yaml_err() {
        let yaml = yaml_doc!(
            r#"
                invalid_key: "value"
            "#
        );
        assert!(StatsdBackend::parse_udp_yaml(&yaml).is_err());

        let yaml = yaml_doc!(
            r#"
                address: "invalid-addr"
            "#
        );
        assert!(StatsdBackend::parse_udp_yaml(&yaml).is_err());

        let yaml = yaml_doc!(
            r#"
                address: "127.0.0.1:8125"
                bind_ip: "invalid-ip"
            "#
        );
        assert!(StatsdBackend::parse_udp_yaml(&yaml).is_err());

        let yaml = yaml_doc!(
            r#"
                bind_ip: "127.0.0.1"
            "#
        );
        assert!(StatsdBackend::parse_udp_yaml(&yaml).is_err());

        let yaml = Yaml::Array(vec![]);
        assert!(StatsdBackend::parse_udp_yaml(&yaml).is_err());

        let yaml = Yaml::Integer(123);
        assert!(StatsdBackend::parse_udp_yaml(&yaml).is_err());
    }

    #[cfg(unix)]
    #[test]
    fn parse_unix_yaml_err() {
        let yaml = yaml_doc!(
            r#"
                invalid_key: "value"
            "#
        );
        assert!(StatsdBackend::parse_unix_yaml(&yaml).is_err());

        let yaml = yaml_doc!(
            r#"
                path: "relative/path"
            "#
        );
        assert!(StatsdBackend::parse_unix_yaml(&yaml).is_err());

        let yaml = yaml_doc!(
            r#"
                path:
            "#
        );
        assert!(StatsdBackend::parse_unix_yaml(&yaml).is_err());

        let yaml = Yaml::Boolean(true);
        assert!(StatsdBackend::parse_unix_yaml(&yaml).is_err());

        let yaml = Yaml::Null;
        assert!(StatsdBackend::parse_unix_yaml(&yaml).is_err());
    }

    #[test]
    fn parse_yaml_ok() {
        let yaml = yaml_doc!(
            r#"
                target_udp: "127.0.0.1:8125"
                prefix: "myapp"
                cache_size: "512KB"
                max_segment_size: "1KB"
                emit_duration: "500ms"
            "#
        );
        let config = StatsdClientConfig::parse_yaml(&yaml, default_node_name()).unwrap();
        match config.backend {
            StatsdBackend::Udp(addr, bind) => {
                assert_eq!(addr, SocketAddr::from_str("127.0.0.1:8125").unwrap());
                assert_eq!(bind, None);
            }
            #[cfg(unix)]
            _ => panic!("expected UDP backend"),
        }
        assert_eq!(config.prefix, NodeName::from_str("myapp").unwrap());
        assert_eq!(config.cache_size, 512 * 1000);
        assert_eq!(config.max_segment_size, Some(1000));
        assert_eq!(config.emit_interval, Duration::from_millis(500));

        let yaml = yaml_doc!(
            r#"
                backend_udp:
                  address: "192.168.1.1:9125"
                  bind_ip: "127.0.0.1"
                prefix: "test.prefix"
                cache_size: 1024
                emit_interval: "1s"
            "#
        );
        let config = StatsdClientConfig::parse_yaml(&yaml, default_node_name()).unwrap();
        match config.backend {
            StatsdBackend::Udp(addr, bind) => {
                assert_eq!(addr, SocketAddr::from_str("192.168.1.1:9125").unwrap());
                assert_eq!(
                    bind,
                    Some(IpAddr::V4(Ipv4Addr::from_str("127.0.0.1").unwrap()))
                );
            }
            #[cfg(unix)]
            _ => panic!("expected UDP backend"),
        }
        assert_eq!(config.prefix, NodeName::from_str("test.prefix").unwrap());
        assert_eq!(config.cache_size, 1024);
        assert_eq!(config.max_segment_size, None);
        assert_eq!(config.emit_interval, Duration::from_secs(1));

        let yaml = yaml_doc!(
            r#"
                target:
                  udp:
                    addr: "10.0.0.1:8126"
                    bind: "0.0.0.0"
                prefix: "nested.udp"
            "#
        );
        let config = StatsdClientConfig::parse_yaml(&yaml, default_node_name()).unwrap();
        match config.backend {
            StatsdBackend::Udp(addr, bind) => {
                assert_eq!(addr, SocketAddr::from_str("10.0.0.1:8126").unwrap());
                assert_eq!(bind, Some(IpAddr::V4(Ipv4Addr::UNSPECIFIED)));
            }
            #[cfg(unix)]
            _ => panic!("expected UDP backend"),
        }
        assert_eq!(config.prefix, NodeName::from_str("nested.udp").unwrap());

        #[cfg(unix)]
        {
            let yaml = yaml_doc!(
                r#"
                    target_unix: "/tmp/statsd.sock"
                    prefix: "unix.app"
                "#
            );
            let config = StatsdClientConfig::parse_yaml(&yaml, default_node_name()).unwrap();
            match config.backend {
                StatsdBackend::Unix(path) => {
                    assert_eq!(path, PathBuf::from("/tmp/statsd.sock"));
                }
                _ => panic!("expected Unix backend"),
            }
            assert_eq!(config.prefix, NodeName::from_str("unix.app").unwrap());

            let yaml = yaml_doc!(
                r#"
                    backend_unix:
                      path: "/var/run/statsd.sock"
                    cache_size: "2MB"
                "#
            );
            let config = StatsdClientConfig::parse_yaml(&yaml, default_node_name()).unwrap();
            match config.backend {
                StatsdBackend::Unix(path) => {
                    assert_eq!(path, PathBuf::from("/var/run/statsd.sock"));
                }
                _ => panic!("expected Unix backend"),
            }
            assert_eq!(config.cache_size, 2 * 1000 * 1000);

            let yaml = yaml_doc!(
                r#"
                    backend:
                      unix:
                        path: "/tmp/nested.sock"
                    prefix: "nested.unix"
                "#
            );
            let config = StatsdClientConfig::parse_yaml(&yaml, default_node_name()).unwrap();
            match config.backend {
                StatsdBackend::Unix(path) => {
                    assert_eq!(path, PathBuf::from("/tmp/nested.sock"));
                }
                _ => panic!("expected Unix backend"),
            }
            assert_eq!(config.prefix, NodeName::from_str("nested.unix").unwrap());
        }
    }

    #[test]
    fn parse_yaml_err() {
        let yaml = yaml_doc!(
            r#"
                invalid_key: "value"
            "#
        );
        assert!(StatsdClientConfig::parse_yaml(&yaml, default_node_name()).is_err());

        let yaml = yaml_doc!(
            r#"
                target_udp: "invalid-address"
            "#
        );
        assert!(StatsdClientConfig::parse_yaml(&yaml, default_node_name()).is_err());

        let yaml = yaml_doc!(
            r#"
                backend_udp: false
            "#
        );
        assert!(StatsdClientConfig::parse_yaml(&yaml, default_node_name()).is_err());

        #[cfg(unix)]
        {
            let yaml = yaml_doc!(
                r#"
                    target_unix: "relative/path"
                "#
            );
            assert!(StatsdClientConfig::parse_yaml(&yaml, default_node_name()).is_err());

            let yaml = yaml_doc!(
                r#"
                    backend_unix: 123
                "#
            );
            assert!(StatsdClientConfig::parse_yaml(&yaml, default_node_name()).is_err());
        }

        let yaml = yaml_doc!(
            r#"
                target: "not_a_map"
            "#
        );
        assert!(StatsdClientConfig::parse_yaml(&yaml, default_node_name()).is_err());

        let yaml = yaml_doc!(
            r#"
                backend:
                  invalid_backend: "value"
            "#
        );
        assert!(StatsdClientConfig::parse_yaml(&yaml, default_node_name()).is_err());

        let yaml = yaml_doc!(
            r#"
                prefix: 123
            "#
        );
        assert!(StatsdClientConfig::parse_yaml(&yaml, default_node_name()).is_err());

        let yaml = yaml_doc!(
            r#"
                cache_size: -1
            "#
        );
        assert!(StatsdClientConfig::parse_yaml(&yaml, default_node_name()).is_err());

        let yaml = yaml_doc!(
            r#"
                max_segment_size: -100
            "#
        );
        assert!(StatsdClientConfig::parse_yaml(&yaml, default_node_name()).is_err());

        let yaml = yaml_doc!(
            r#"
                emit_interval: "1xs"
            "#
        );
        assert!(StatsdClientConfig::parse_yaml(&yaml, default_node_name()).is_err());

        let yaml = Yaml::Array(vec![]);
        assert!(StatsdClientConfig::parse_yaml(&yaml, default_node_name()).is_err());

        let yaml = Yaml::Integer(123);
        assert!(StatsdClientConfig::parse_yaml(&yaml, default_node_name()).is_err());

        let yaml = Yaml::Boolean(true);
        assert!(StatsdClientConfig::parse_yaml(&yaml, default_node_name()).is_err());

        let yaml = Yaml::Real("1.23".to_string());
        assert!(StatsdClientConfig::parse_yaml(&yaml, default_node_name()).is_err());

        let yaml = Yaml::Null;
        assert!(StatsdClientConfig::parse_yaml(&yaml, default_node_name()).is_err());
    }
}
