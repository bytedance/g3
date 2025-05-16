/*
 * SPDX-License-Identifier: Apache-2.0
 * Copyright 2023-2025 ByteDance and/or its affiliates.
 */

use anyhow::anyhow;
use url::Url;
use yaml_rust::{Yaml, yaml};

use g3_types::metrics::NodeName;
use g3_yaml::YamlDocPosition;

use super::KeyStoreConfig;

#[derive(Clone, Debug, PartialEq)]
pub struct RedisKeyStoreConfig {
    name: NodeName,
    position: Option<YamlDocPosition>,
    url: Option<Url>,
}

impl RedisKeyStoreConfig {
    fn new(position: Option<YamlDocPosition>) -> Self {
        RedisKeyStoreConfig {
            name: NodeName::default(),
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
                self.name = g3_yaml::value::as_metric_node_name(v)?;
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

impl KeyStoreConfig for RedisKeyStoreConfig {
    #[inline]
    fn name(&self) -> &NodeName {
        &self.name
    }

    async fn load_keys(&self) -> anyhow::Result<()> {
        unimplemented!()
    }
}
