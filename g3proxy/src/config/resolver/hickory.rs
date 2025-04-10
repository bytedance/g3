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

use std::collections::BTreeSet;
use std::net::IpAddr;

use anyhow::anyhow;
use yaml_rust::{Yaml, yaml};

use g3_resolver::driver::hickory::HickoryDriverConfig;
use g3_resolver::{AnyResolveDriverConfig, ResolverRuntimeConfig};
use g3_socket::BindAddr;
use g3_types::metrics::NodeName;
use g3_yaml::YamlDocPosition;

use super::{AnyResolverConfig, ResolverConfigDiffAction};

const RESOLVER_CONFIG_TYPE: &str = "hickory";

#[derive(Clone, PartialEq)]
pub(crate) struct HickoryResolverConfig {
    name: NodeName,
    position: Option<YamlDocPosition>,
    runtime: ResolverRuntimeConfig,
    driver: HickoryDriverConfig,
}

impl From<&HickoryResolverConfig> for g3_resolver::ResolverConfig {
    fn from(c: &HickoryResolverConfig) -> Self {
        g3_resolver::ResolverConfig {
            name: c.name.to_string(),
            runtime: c.runtime.clone(),
            driver: AnyResolveDriverConfig::Hickory(Box::new(c.driver.clone())),
        }
    }
}

impl HickoryResolverConfig {
    fn new(position: Option<YamlDocPosition>) -> Self {
        HickoryResolverConfig {
            name: NodeName::default(),
            position,
            runtime: Default::default(),
            driver: Default::default(),
        }
    }

    #[inline]
    pub(crate) fn get_bind_addr(&self) -> BindAddr {
        self.driver.get_bind_addr()
    }

    #[inline]
    pub(crate) fn get_servers(&self) -> Vec<IpAddr> {
        self.driver.get_servers()
    }

    #[inline]
    pub(crate) fn get_server_port(&self) -> Option<u16> {
        self.driver.get_server_port()
    }

    pub(crate) fn get_encryption_summary(&self) -> Option<String> {
        self.driver.get_encryption().map(|c| c.summary())
    }

    pub(crate) fn parse(
        map: &yaml::Hash,
        position: Option<YamlDocPosition>,
    ) -> anyhow::Result<Self> {
        let mut resolver = Self::new(position);

        g3_yaml::foreach_kv(map, |k, v| resolver.set(k, v))?;

        resolver.check()?;
        Ok(resolver)
    }

    fn set(&mut self, k: &str, v: &Yaml) -> anyhow::Result<()> {
        match g3_yaml::key::normalize(k).as_str() {
            super::CONFIG_KEY_RESOLVER_TYPE => Ok(()),
            super::CONFIG_KEY_RESOLVER_NAME => {
                self.name = g3_yaml::value::as_metric_node_name(v)?;
                Ok(())
            }
            "graceful_stop_wait" => {
                self.runtime.graceful_stop_wait = g3_yaml::humanize::as_duration(v)?;
                Ok(())
            }
            "protective_query_timeout" => {
                self.runtime.protective_query_timeout = g3_yaml::humanize::as_duration(v)?;
                Ok(())
            }
            _ => {
                let lookup_dir = g3_daemon::config::get_lookup_dir(self.position.as_ref())?;
                self.driver.set_by_yaml_kv(k, v, Some(lookup_dir))
            }
        }
    }

    fn check(&mut self) -> anyhow::Result<()> {
        if self.name.is_empty() {
            return Err(anyhow!("name is not set"));
        }
        self.driver.check()
    }
}

impl super::ResolverConfig for HickoryResolverConfig {
    fn name(&self) -> &NodeName {
        &self.name
    }

    fn position(&self) -> Option<YamlDocPosition> {
        self.position.clone()
    }

    fn r#type(&self) -> &'static str {
        RESOLVER_CONFIG_TYPE
    }

    fn diff_action(&self, new: &AnyResolverConfig) -> ResolverConfigDiffAction {
        let AnyResolverConfig::Hickory(new) = new else {
            return ResolverConfigDiffAction::SpawnNew;
        };

        if self.eq(new) {
            return ResolverConfigDiffAction::NoAction;
        }

        ResolverConfigDiffAction::Update
    }

    fn dependent_resolver(&self) -> Option<BTreeSet<NodeName>> {
        None
    }
}
