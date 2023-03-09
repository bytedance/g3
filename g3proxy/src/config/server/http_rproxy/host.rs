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

use std::sync::Arc;

use anyhow::{anyhow, Context};
use yaml_rust::Yaml;

use g3_types::net::RustlsServerConfigBuilder;
use g3_types::route::UriPathMatch;
use g3_yaml::{YamlDocPosition, YamlMapCallback};

use super::HttpServiceConfig;

#[derive(Default, Debug, PartialEq)]
pub(crate) struct HttpHostConfig {
    pub(crate) sites: UriPathMatch<Arc<HttpServiceConfig>>,
    pub(crate) tls_server_builder: Option<RustlsServerConfigBuilder>,
}

impl YamlMapCallback for HttpHostConfig {
    fn type_name(&self) -> &'static str {
        "HttpLocalSiteConfig"
    }

    fn parse_kv(
        &mut self,
        key: &str,
        value: &Yaml,
        doc: Option<&YamlDocPosition>,
    ) -> anyhow::Result<()> {
        match key {
            "services" => {
                self.sites = g3_yaml::value::as_url_path_matched_obj(value, doc).context(
                    format!("invalid url path matched HttpSiteConfig value for key {key}"),
                )?;
                Ok(())
            }
            "tls_server" => {
                let lookup_dir = crate::config::get_lookup_dir(doc);
                let builder =
                    g3_yaml::value::as_rustls_server_config_builder(value, Some(&lookup_dir))
                        .context(format!(
                            "invalid tls server config builder value for key {key}"
                        ))?;
                self.tls_server_builder = Some(builder);
                Ok(())
            }
            _ => Err(anyhow!("invalid key {key}")),
        }
    }
}
