/*
 * SPDX-License-Identifier: Apache-2.0
 * Copyright 2023-2025 ByteDance and/or its affiliates.
 */

use anyhow::{Context, anyhow};
use yaml_rust::Yaml;

use g3_types::net::{Host, OpensslClientConfigBuilder, RustlsServerConfigBuilder, UpstreamAddr};
use g3_yaml::{YamlDocPosition, YamlMapCallback};

#[derive(Debug, PartialEq)]
pub(crate) struct HttpHostConfig {
    upstream: UpstreamAddr,
    pub(crate) tls_server_builder: Option<RustlsServerConfigBuilder>,
    pub(crate) tls_client_builder: Option<OpensslClientConfigBuilder>,
    pub(crate) tls_name: Host,
}

impl Default for HttpHostConfig {
    fn default() -> Self {
        HttpHostConfig {
            upstream: UpstreamAddr::empty(),
            tls_server_builder: None,
            tls_client_builder: None,
            tls_name: Host::empty(),
        }
    }
}

impl HttpHostConfig {
    pub(crate) fn upstream(&self) -> &UpstreamAddr {
        &self.upstream
    }
}

impl YamlMapCallback for HttpHostConfig {
    fn type_name(&self) -> &'static str {
        "HttpHostConfig"
    }

    fn parse_kv(
        &mut self,
        key: &str,
        value: &Yaml,
        doc: Option<&YamlDocPosition>,
    ) -> anyhow::Result<()> {
        match key {
            "upstream" => {
                self.upstream = g3_yaml::value::as_upstream_addr(value, 80)
                    .context(format!("invalid upstream addr value for key {key}"))?;
                Ok(())
            }
            "tls_server" => {
                let lookup_dir = g3_daemon::config::get_lookup_dir(doc)?;
                let builder =
                    g3_yaml::value::as_rustls_server_config_builder(value, Some(lookup_dir))
                        .context(format!(
                            "invalid tls server config builder value for key {key}"
                        ))?;
                self.tls_server_builder = Some(builder);
                Ok(())
            }
            "tls_client" => {
                let lookup_dir = g3_daemon::config::get_lookup_dir(doc)?;
                let builder = g3_yaml::value::as_to_one_openssl_tls_client_config_builder(
                    value,
                    Some(lookup_dir),
                )
                .context(format!(
                    "invalid openssl tls client config value for key {key}"
                ))?;
                self.tls_client_builder = Some(builder);
                Ok(())
            }
            "tls_name" => {
                self.tls_name = g3_yaml::value::as_host(value)
                    .context(format!("invalid tls name value for key {key}"))?;
                Ok(())
            }
            _ => Err(anyhow!("invalid key {key}")),
        }
    }

    fn check(&mut self) -> anyhow::Result<()> {
        if self.upstream.is_empty() {
            return Err(anyhow!("upstream is empty"));
        }
        if self.tls_name.is_empty() {
            self.upstream.host().clone_into(&mut self.tls_name);
        }
        Ok(())
    }
}
