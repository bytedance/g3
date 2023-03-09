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

use g3_types::net::OpensslTlsClientConfigBuilder;
use g3_types::net::UpstreamAddr;
use g3_yaml::{YamlDocPosition, YamlMapCallback};

#[derive(Debug, PartialEq)]
pub(crate) struct HttpServiceConfig {
    upstream: UpstreamAddr,
    pub(crate) tls_client_builder: Option<OpensslTlsClientConfigBuilder>,
    pub(crate) tls_name: String,
}

impl Default for HttpServiceConfig {
    fn default() -> Self {
        let upstream = UpstreamAddr::empty();
        HttpServiceConfig::new(upstream)
    }
}

impl HttpServiceConfig {
    fn new(upstream: UpstreamAddr) -> Self {
        let tls_name = upstream.host().to_string();
        HttpServiceConfig {
            upstream,
            tls_client_builder: None,
            tls_name,
        }
    }

    fn check(&mut self) -> anyhow::Result<()> {
        if self.upstream.is_empty() {
            return Err(anyhow!("upstream is empty"));
        }

        Ok(())
    }

    pub(crate) fn upstream(&self) -> &UpstreamAddr {
        &self.upstream
    }
}

impl YamlMapCallback for HttpServiceConfig {
    fn type_name(&self) -> &'static str {
        "HttpSiteConfig"
    }

    #[inline]
    fn parse_kv(
        &mut self,
        key: &str,
        value: &Yaml,
        doc: Option<&YamlDocPosition>,
    ) -> anyhow::Result<()> {
        match g3_yaml::key::normalize(key).as_str() {
            "upstream" => {
                self.upstream = g3_yaml::value::as_upstream_addr(value, 80)
                    .context(format!("invalid upstream addr value for key {key}"))?;
                Ok(())
            }
            "tls_client" => {
                let lookup_dir = crate::config::get_lookup_dir(doc);
                let builder = g3_yaml::value::as_to_one_openssl_tls_client_config_builder(
                    value,
                    Some(&lookup_dir),
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

    #[inline]
    fn check(&mut self) -> anyhow::Result<()> {
        self.check()
    }
}
