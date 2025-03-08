/*
 * Copyright 2025 ByteDance and/or its affiliates.
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

use anyhow::{Context, anyhow};
use yaml_rust::{Yaml, yaml};

use g3_types::acl::AclNetworkRuleBuilder;
use g3_types::metrics::NodeName;
use g3_types::net::UdpListenConfig;
use g3_yaml::YamlDocPosition;

use super::{AnyInputConfig, InputConfig, InputConfigDiffAction};

const INPUT_CONFIG_TYPE: &str = "StatsD";

#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) struct StatsdInputConfig {
    name: NodeName,
    position: Option<YamlDocPosition>,
    pub(crate) listen: UdpListenConfig,
    pub(crate) listen_in_worker: bool,
    pub(crate) ingress_net_filter: Option<AclNetworkRuleBuilder>,
}

impl StatsdInputConfig {
    fn new(position: Option<YamlDocPosition>) -> Self {
        StatsdInputConfig {
            name: NodeName::default(),
            position,
            listen: UdpListenConfig::default(),
            listen_in_worker: false,
            ingress_net_filter: None,
        }
    }

    pub(crate) fn parse(
        map: &yaml::Hash,
        position: Option<YamlDocPosition>,
    ) -> anyhow::Result<Self> {
        let mut input = StatsdInputConfig::new(position);

        g3_yaml::foreach_kv(map, |k, v| input.set(k, v))?;

        input.check()?;
        Ok(input)
    }

    fn set(&mut self, k: &str, v: &Yaml) -> anyhow::Result<()> {
        match g3_yaml::key::normalize(k).as_str() {
            super::CONFIG_KEY_INPUT_TYPE => Ok(()),
            super::CONFIG_KEY_INPUT_NAME => {
                self.name = g3_yaml::value::as_metrics_name(v)?;
                Ok(())
            }
            "listen" => {
                self.listen = g3_yaml::value::as_udp_listen_config(v)
                    .context(format!("invalid udp listen config value for key {k}"))?;
                Ok(())
            }
            "listen_in_worker" => {
                self.listen_in_worker = g3_yaml::value::as_bool(v)?;
                Ok(())
            }
            "ingress_network_filter" | "ingress_net_filter" => {
                let filter = g3_yaml::value::acl::as_ingress_network_rule_builder(v).context(
                    format!("invalid ingress network acl rule value for key {k}"),
                )?;
                self.ingress_net_filter = Some(filter);
                Ok(())
            }
            _ => Err(anyhow!("invalid key {k}")),
        }
    }

    fn check(&mut self) -> anyhow::Result<()> {
        if self.name.is_empty() {
            return Err(anyhow!("name is not set"));
        }
        // make sure listen is always set
        self.listen.check().context("invalid listen config")?;

        Ok(())
    }
}

impl InputConfig for StatsdInputConfig {
    fn name(&self) -> &NodeName {
        &self.name
    }

    fn position(&self) -> Option<YamlDocPosition> {
        self.position.clone()
    }

    fn input_type(&self) -> &'static str {
        INPUT_CONFIG_TYPE
    }

    fn diff_action(&self, new: &AnyInputConfig) -> InputConfigDiffAction {
        let AnyInputConfig::StatsD(new) = new else {
            return InputConfigDiffAction::SpawnNew;
        };

        if self.eq(new) {
            return InputConfigDiffAction::NoAction;
        }

        if self.listen != new.listen {
            return InputConfigDiffAction::ReloadAndRespawn;
        }

        InputConfigDiffAction::ReloadOnlyConfig
    }
}
