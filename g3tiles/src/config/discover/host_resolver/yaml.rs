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
use yaml_rust::{yaml, Yaml};

use g3_yaml::YamlDocPosition;

use super::{HostResolverDiscoverConfig, HostResolverDiscoverInput};

impl HostResolverDiscoverConfig {
    pub(crate) fn parse_yaml_conf(
        map: &yaml::Hash,
        position: Option<YamlDocPosition>,
    ) -> anyhow::Result<Self> {
        let mut site = HostResolverDiscoverConfig::new(position);
        g3_yaml::foreach_kv(map, |k, v| site.set_yaml(k, v))?;
        site.check()?;
        Ok(site)
    }

    fn set_yaml(&mut self, k: &str, v: &Yaml) -> anyhow::Result<()> {
        match k {
            super::CONFIG_KEY_DISCOVER_TYPE => Ok(()),
            super::CONFIG_KEY_DISCOVER_NAME => {
                self.name = g3_yaml::value::as_metrics_name(v)?;
                Ok(())
            }
            _ => Err(anyhow!("invalid key {k}")),
        }
    }

    pub(crate) fn parse_yaml_data(
        &self,
        input: &Yaml,
    ) -> anyhow::Result<HostResolverDiscoverInput> {
        let addr = g3_yaml::value::as_upstream_addr(input, 0)?;
        Ok(HostResolverDiscoverInput { addr })
    }
}
