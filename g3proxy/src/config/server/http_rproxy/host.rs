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

use anyhow::{anyhow, Context};
use yaml_rust::Yaml;

use g3_types::net::{OpensslTlsClientConfigBuilder, RustlsServerConfigBuilder, UpstreamAddr};
use g3_yaml::{YamlDocPosition, YamlMapCallback};

#[derive(Debug, PartialEq)]
pub(crate) struct HttpHostConfig {
    upstream: UpstreamAddr,
    pub(crate) tls_server_builder: Option<RustlsServerConfigBuilder>,
    pub(crate) tls_client_builder: Option<OpensslTlsClientConfigBuilder>,
    pub(crate) tls_name: String,
}

impl Default for HttpHostConfig {
    fn default() -> Self {
        HttpHostConfig {
            upstream: UpstreamAddr::empty(),
            tls_server_builder: None,
            tls_client_builder: None,
            tls_name: String::new(),
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
                self.tls_name = self.upstream.host().to_string();
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
                self.tls_name = g3_yaml::value::as_string(value)
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
        Ok(())
    }
}
