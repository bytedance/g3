/*
 * SPDX-License-Identifier: Apache-2.0
 * Copyright 2024-2025 ByteDance and/or its affiliates.
 */

use anyhow::{Context, anyhow};
use yaml_rust::{Yaml, yaml};

use g3_yaml::YamlDocPosition;

use super::{StaticAddrDiscoverConfig, StaticAddrDiscoverInput};

impl StaticAddrDiscoverConfig {
    pub(crate) fn parse_yaml_conf(
        map: &yaml::Hash,
        position: Option<YamlDocPosition>,
    ) -> anyhow::Result<Self> {
        let mut site = StaticAddrDiscoverConfig::new(position);
        g3_yaml::foreach_kv(map, |k, v| site.set_yaml(k, v))?;
        site.check()?;
        Ok(site)
    }

    fn set_yaml(&mut self, k: &str, v: &Yaml) -> anyhow::Result<()> {
        match k {
            super::CONFIG_KEY_DISCOVER_TYPE => Ok(()),
            super::CONFIG_KEY_DISCOVER_NAME => {
                self.name = g3_yaml::value::as_metric_node_name(v)?;
                Ok(())
            }
            _ => Err(anyhow!("invalid key {k}")),
        }
    }

    pub(crate) fn parse_yaml_data(&self, input: &Yaml) -> anyhow::Result<StaticAddrDiscoverInput> {
        let mut parsed = StaticAddrDiscoverInput::default();
        match input {
            Yaml::Array(seq) => {
                for (i, v) in seq.iter().enumerate() {
                    let data = g3_yaml::value::as_weighted_sockaddr(v)
                        .context(format!("invalid weighted socket address value for #{i}"))?;
                    parsed.inner.push(data);
                }
            }
            v => {
                let data = g3_yaml::value::as_weighted_sockaddr(v)
                    .context("invalid weighted socket address value")?;
                parsed.inner.push(data);
            }
        }
        Ok(parsed)
    }
}
