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

use anyhow::anyhow;
use async_trait::async_trait;
use openssl::pkey::{PKey, Private};
use url::Url;
use yaml_rust::{yaml, Yaml};

use g3_types::metrics::MetricsName;
use g3_yaml::YamlDocPosition;

use super::KeyStoreConfig;

#[derive(Clone, Debug, PartialEq)]
pub struct RedisKeyStoreConfig {
    name: MetricsName,
    position: Option<YamlDocPosition>,
    url: Option<Url>,
}

impl RedisKeyStoreConfig {
    fn new(position: Option<YamlDocPosition>) -> Self {
        RedisKeyStoreConfig {
            name: MetricsName::default(),
            position,
            url: None,
        }
    }

    pub(super) fn parse(
        map: &yaml::Hash,
        position: Option<YamlDocPosition>,
    ) -> anyhow::Result<Self> {
        let mut server = RedisKeyStoreConfig::new(position);

        g3_yaml::foreach_kv(map, |k, v| server.set(k, v))?;

        server.check()?;
        Ok(server)
    }

    fn check(&self) -> anyhow::Result<()> {
        if self.name.is_empty() {
            return Err(anyhow!("name is not set"));
        }
        if self.url.is_none() {
            return Err(anyhow!("url is not set"));
        }
        Ok(())
    }

    fn set(&mut self, k: &str, v: &Yaml) -> anyhow::Result<()> {
        match g3_yaml::key::normalize(k).as_str() {
            super::CONFIG_KEY_STORE_TYPE => Ok(()),
            "name" => {
                self.name = g3_yaml::value::as_metrics_name(v)?;
                Ok(())
            }
            "url" => {
                let url = g3_yaml::value::as_url(v)?;
                self.url = Some(url);
                Ok(())
            }
            _ => Err(anyhow!("invalid key {k}")),
        }
    }
}

#[async_trait]
impl KeyStoreConfig for RedisKeyStoreConfig {
    #[inline]
    fn name(&self) -> &MetricsName {
        &self.name
    }

    async fn load_certs(&self) -> anyhow::Result<Vec<PKey<Private>>> {
        unimplemented!()
    }
}
