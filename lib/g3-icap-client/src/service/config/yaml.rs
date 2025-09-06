/*
 * SPDX-License-Identifier: Apache-2.0
 * Copyright 2023-2025 ByteDance and/or its affiliates.
 */

use std::path::Path;
use std::str::FromStr;

use anyhow::{Context, anyhow};
use url::Url;
use yaml_rust::{Yaml, yaml};

use super::{IcapMethod, IcapServiceConfig};

impl IcapServiceConfig {
    fn parse_yaml(
        map: &yaml::Hash,
        method: IcapMethod,
        lookup_dir: Option<&Path>,
    ) -> anyhow::Result<Self> {
        const KEY_URL: &str = "url";
        let url = g3_yaml::hash_get_required(map, KEY_URL)?;
        let url = g3_yaml::value::as_url(url)
            .context(format!("invalid url string value for key {KEY_URL}"))?;
        let mut config = IcapServiceConfig::new(method, url)?;

        g3_yaml::foreach_kv(map, |k, v| match g3_yaml::key::normalize(k).as_str() {
            KEY_URL => Ok(()),
            "tls_client" => {
                let tls_client = g3_yaml::value::as_rustls_client_config_builder(v, lookup_dir)
                    .context(format!(
                        "invalid rustls tls client config value for key {k}"
                    ))?;
                config.set_tls_client(tls_client);
                Ok(())
            }
            "tls_name" => {
                let tls_name = g3_yaml::value::as_rustls_server_name(v)
                    .context(format!("invalid rustls server name value for key {k}"))?;
                config.set_tls_name(tls_name);
                Ok(())
            }
            "tcp_keepalive" => {
                let keepalive = g3_yaml::value::as_tcp_keepalive_config(v)
                    .context(format!("invalid tcp keepalive config value for key {k}"))?;
                config.set_tcp_keepalive(keepalive);
                Ok(())
            }
            #[cfg(unix)]
            "use_unix_socket" => {
                let path = g3_yaml::value::as_absolute_path(v)
                    .context(format!("invalid absolute path value for key {k}"))?;
                config.use_unix_socket = Some(path);
                Ok(())
            }
            "icap_connection_pool" | "connection_pool" | "pool" => {
                config.connection_pool = g3_yaml::value::as_connection_pool_config(v)
                    .context(format!("invalid connection pool config value for key {k}"))?;
                Ok(())
            }
            "icap_max_header_size" => {
                let size = g3_yaml::humanize::as_usize(v)
                    .context(format!("invalid humanize usize value for key {k}"))?;
                config.set_icap_max_header_size(size);
                Ok(())
            }
            "disable_preview" | "no_preview" => {
                config.disable_preview = g3_yaml::value::as_bool(v)?;
                Ok(())
            }
            "preview_data_read_timeout" => {
                let time = g3_yaml::humanize::as_duration(v)
                    .context(format!("invalid humanize duration value for key {k}"))?;
                config.set_preview_data_read_timeout(time);
                Ok(())
            }
            "respond_shared_names" => {
                if let Yaml::Array(seq) = v {
                    for (i, v) in seq.iter().enumerate() {
                        let name = g3_yaml::value::as_http_header_name(v)
                            .context(format!("invalid http header name value for key {k}#{i}"))?;
                        config.add_respond_shared_name(name);
                    }
                } else {
                    let name = g3_yaml::value::as_http_header_name(v)
                        .context(format!("invalid http header name value for key {k}"))?;
                    config.add_respond_shared_name(name);
                }
                Ok(())
            }
            "bypass" => {
                let bypass = g3_yaml::value::as_bool(v)?;
                config.set_bypass(bypass);
                Ok(())
            }
            _ => Err(anyhow!("invalid key {k}")),
        })?;

        Ok(config)
    }

    pub fn parse_reqmod_service_yaml(
        value: &Yaml,
        lookup_dir: Option<&Path>,
    ) -> anyhow::Result<Self> {
        match value {
            Yaml::Hash(map) => Self::parse_yaml(map, IcapMethod::Reqmod, lookup_dir),
            Yaml::String(s) => {
                let url = Url::from_str(s).map_err(|e| anyhow!("invalid url string: {e}"))?;
                IcapServiceConfig::new(IcapMethod::Reqmod, url)
            }
            _ => Err(anyhow!(
                "yaml value type for 'icap service config' should be 'map' or 'url str'"
            )),
        }
    }

    pub fn parse_respmod_service_yaml(
        value: &Yaml,
        lookup_dir: Option<&Path>,
    ) -> anyhow::Result<Self> {
        match value {
            Yaml::Hash(map) => Self::parse_yaml(map, IcapMethod::Respmod, lookup_dir),
            Yaml::String(s) => {
                let url = Url::from_str(s).map_err(|e| anyhow!("invalid url string: {e}"))?;
                IcapServiceConfig::new(IcapMethod::Respmod, url)
            }
            _ => Err(anyhow!(
                "yaml value type for 'icap service config' should be 'map' or 'url str'"
            )),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use g3_yaml::{yaml_doc, yaml_str};
    use yaml_rust::YamlLoader;

    #[test]
    fn parse_reqmod_service_from_string() {
        // Valid ICAP URL
        let yaml = yaml_str!("icap://example.com:1344/service");
        let config = IcapServiceConfig::parse_reqmod_service_yaml(&yaml, None).unwrap();
        assert_eq!(config.method, IcapMethod::Reqmod);
        assert_eq!(config.url.to_string(), "icap://example.com:1344/service");

        // Valid ICAPS URL
        let yaml = yaml_str!("icaps://secure.example.com:1344/service");
        let config = IcapServiceConfig::parse_reqmod_service_yaml(&yaml, None).unwrap();
        assert_eq!(config.method, IcapMethod::Reqmod);
        assert_eq!(
            config.url.to_string(),
            "icaps://secure.example.com:1344/service"
        );
        assert!(config.tls_client.is_some());

        // Invalid URL format
        let yaml = yaml_str!("invalid-url");
        assert!(IcapServiceConfig::parse_reqmod_service_yaml(&yaml, None).is_err());
    }

    #[test]
    fn parse_respmod_service_from_string() {
        // Valid ICAP URL
        let yaml = yaml_str!("icap://example.com:1344/service");
        let config = IcapServiceConfig::parse_respmod_service_yaml(&yaml, None).unwrap();
        assert_eq!(config.method, IcapMethod::Respmod);
        assert_eq!(config.url.to_string(), "icap://example.com:1344/service");

        // Valid ICAPS URL
        let yaml = yaml_str!("icaps://secure.example.com:1344/service");
        let config = IcapServiceConfig::parse_respmod_service_yaml(&yaml, None).unwrap();
        assert_eq!(config.method, IcapMethod::Respmod);
        assert_eq!(
            config.url.to_string(),
            "icaps://secure.example.com:1344/service"
        );
        assert!(config.tls_client.is_some());

        // Invalid URL scheme
        let yaml = yaml_str!("https://example.com:1344/service");
        assert!(IcapServiceConfig::parse_respmod_service_yaml(&yaml, None).is_err());
    }

    #[test]
    fn parse_reqmod_service_from_map() {
        let yaml = yaml_doc!(
            r#"
                url: "icap://example.com:1344/service"
                tls_name: "example.com"
                tcp_keepalive:
                  idle_time: 60
                  probe_interval: 10
                  probe_count: 3
                icap_connection_pool:
                  max_idle_count: 10
                  idle_timeout: 30s
                disable_preview: true
                respond_shared_names:
                  - "X-Header-1"
                  - "X-Header-2"
                bypass: false
            "#
        );
        let config = IcapServiceConfig::parse_reqmod_service_yaml(&yaml, None).unwrap();
        assert_eq!(config.method, IcapMethod::Reqmod);
        assert_eq!(config.url.to_string(), "icap://example.com:1344/service");
        assert_eq!(
            config.tls_name,
            rustls_pki_types::ServerName::try_from("example.com").unwrap()
        );
        assert_eq!(
            config.tcp_keepalive.idle_time(),
            std::time::Duration::from_secs(60)
        );
        assert_eq!(
            config.tcp_keepalive.probe_interval(),
            Some(std::time::Duration::from_secs(10))
        );
        assert_eq!(config.tcp_keepalive.probe_count(), Some(3));
        assert_eq!(config.connection_pool.max_idle_count(), 10);
        assert_eq!(
            config.connection_pool.idle_timeout(),
            std::time::Duration::from_secs(30)
        );
        assert!(config.disable_preview);
        assert_eq!(config.respond_shared_names.len(), 2);
        assert!(config.respond_shared_names.contains("x-header-1"));
        assert!(config.respond_shared_names.contains("x-header-2"));
        assert!(!config.bypass);

        #[cfg(unix)]
        {
            let yaml = yaml_doc!(
                r#"
                    url: "icap://example.com:1344/service"
                    use_unix_socket: "/tmp/icap.sock"
                    respond_shared_names: []
                "#
            );
            let config = IcapServiceConfig::parse_reqmod_service_yaml(&yaml, None).unwrap();
            assert_eq!(config.method, IcapMethod::Reqmod);
            assert_eq!(
                config.use_unix_socket,
                Some(std::path::PathBuf::from("/tmp/icap.sock"))
            );
            assert!(config.respond_shared_names.is_empty());
        }
    }

    #[test]
    fn parse_respmod_service_from_map() {
        let yaml = yaml_doc!(
            r#"
                url: "icaps://secure.example.com:1344/service"
                tls_client:
                  no_session_cache: true
                tls_name: "secure.example.com"
                connection_pool:
                  check_interval: 15s
                  min_idle_count: 5
                icap_max_header_size: "8KB"
                no_preview: false
                preview_data_read_timeout: "1s"
                respond_shared_names: "X-Single-Header"
                bypass: true
            "#
        );
        let config = IcapServiceConfig::parse_respmod_service_yaml(&yaml, None).unwrap();
        assert_eq!(config.method, IcapMethod::Respmod);
        assert_eq!(
            config.url.to_string(),
            "icaps://secure.example.com:1344/service"
        );
        let mut tls_client = g3_types::net::RustlsClientConfigBuilder::default();
        tls_client.set_no_session_cache();
        assert_eq!(config.tls_client.unwrap(), tls_client);
        assert_eq!(
            config.tls_name,
            rustls_pki_types::ServerName::try_from("secure.example.com").unwrap()
        );
        assert_eq!(
            config.connection_pool.check_interval(),
            std::time::Duration::from_secs(15)
        );
        assert_eq!(config.connection_pool.min_idle_count(), 5);
        assert_eq!(config.icap_max_header_size, 8 * 1000);
        assert!(!config.disable_preview);
        assert_eq!(
            config.preview_data_read_timeout,
            std::time::Duration::from_secs(1)
        );
        assert!(config.respond_shared_names.contains("x-single-header"));
        assert!(config.bypass);
    }

    #[test]
    fn parse_yaml_err() {
        let yaml = Yaml::Array(vec![]);
        assert!(IcapServiceConfig::parse_reqmod_service_yaml(&yaml, None).is_err());

        let yaml = Yaml::Integer(123);
        assert!(IcapServiceConfig::parse_reqmod_service_yaml(&yaml, None).is_err());

        let yaml = Yaml::Boolean(true);
        assert!(IcapServiceConfig::parse_respmod_service_yaml(&yaml, None).is_err());

        let yaml = Yaml::Real("1.23".to_string());
        assert!(IcapServiceConfig::parse_respmod_service_yaml(&yaml, None).is_err());

        let yaml = yaml_doc!(
            r#"
                tls_name: "example.com"
            "#
        );
        assert!(IcapServiceConfig::parse_reqmod_service_yaml(&yaml, None).is_err());

        let yaml = yaml_doc!(
            r#"
                unknown_key: "value"
            "#
        );
        assert!(IcapServiceConfig::parse_reqmod_service_yaml(&yaml, None).is_err());

        let yaml = yaml_doc!(
            r#"
                tls_name: 123
            "#
        );
        assert!(IcapServiceConfig::parse_reqmod_service_yaml(&yaml, None).is_err());

        let yaml = yaml_doc!(
            r#"
                tcp_keepalive: "invalid"
            "#
        );
        assert!(IcapServiceConfig::parse_reqmod_service_yaml(&yaml, None).is_err());

        let yaml = yaml_doc!(
            r#"
                pool: "invalid"
            "#
        );
        assert!(IcapServiceConfig::parse_reqmod_service_yaml(&yaml, None).is_err());

        let yaml = yaml_doc!(
            r#"
                icap_max_header_size: "16XB"
            "#
        );
        assert!(IcapServiceConfig::parse_reqmod_service_yaml(&yaml, None).is_err());

        let yaml = yaml_doc!(
            r#"
                tls_client: 123
            "#
        );
        assert!(IcapServiceConfig::parse_respmod_service_yaml(&yaml, None).is_err());

        let yaml = yaml_doc!(
            r#"
                disable_preview: 0
            "#
        );
        assert!(IcapServiceConfig::parse_respmod_service_yaml(&yaml, None).is_err());

        let yaml = yaml_doc!(
            r#"
                preview_data_read_timeout: "-1s"
            "#
        );
        assert!(IcapServiceConfig::parse_respmod_service_yaml(&yaml, None).is_err());

        let yaml = yaml_doc!(
            r#"
                respond_shared_names: false
            "#
        );
        assert!(IcapServiceConfig::parse_respmod_service_yaml(&yaml, None).is_err());

        let yaml = yaml_doc!(
            r#"
                bypass: "invalid"
            "#
        );
        assert!(IcapServiceConfig::parse_respmod_service_yaml(&yaml, None).is_err());

        #[cfg(unix)]
        {
            let yaml = yaml_doc!(
                r#"
                    use_unix_socket: 123
                "#
            );
            assert!(IcapServiceConfig::parse_respmod_service_yaml(&yaml, None).is_err());
        }
    }
}
