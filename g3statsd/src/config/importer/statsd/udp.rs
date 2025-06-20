/*
 * SPDX-License-Identifier: Apache-2.0
 * Copyright 2025 ByteDance and/or its affiliates.
 */

use anyhow::{Context, anyhow};
use yaml_rust::{Yaml, yaml};

use g3_types::acl::AclNetworkRuleBuilder;
use g3_types::metrics::NodeName;
use g3_types::net::UdpListenConfig;
use g3_yaml::YamlDocPosition;

use super::{AnyImporterConfig, ImporterConfig, ImporterConfigDiffAction};

const IMPORTER_CONFIG_TYPE: &str = "StatsD_UDP";

#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) struct StatsdUdpImporterConfig {
    name: NodeName,
    position: Option<YamlDocPosition>,
    pub(crate) collector: NodeName,
    pub(crate) listen: UdpListenConfig,
    pub(crate) listen_in_worker: bool,
    pub(crate) ingress_net_filter: Option<AclNetworkRuleBuilder>,
}

impl StatsdUdpImporterConfig {
    fn new(position: Option<YamlDocPosition>) -> Self {
        StatsdUdpImporterConfig {
            name: NodeName::default(),
            position,
            collector: Default::default(),
            listen: UdpListenConfig::default(),
            listen_in_worker: false,
            ingress_net_filter: None,
        }
    }

    pub(crate) fn parse(
        map: &yaml::Hash,
        position: Option<YamlDocPosition>,
    ) -> anyhow::Result<Self> {
        let mut importer = StatsdUdpImporterConfig::new(position);

        g3_yaml::foreach_kv(map, |k, v| importer.set(k, v))?;

        importer.check()?;
        Ok(importer)
    }

    fn set(&mut self, k: &str, v: &Yaml) -> anyhow::Result<()> {
        match g3_yaml::key::normalize(k).as_str() {
            super::CONFIG_KEY_IMPORTER_TYPE => Ok(()),
            super::CONFIG_KEY_IMPORTER_NAME => {
                self.name = g3_yaml::value::as_metric_node_name(v)?;
                Ok(())
            }
            "collector" => {
                self.collector = g3_yaml::value::as_metric_node_name(v)?;
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
        if self.collector.is_empty() {
            return Err(anyhow!("collector is not set"));
        }
        // make sure listen is always set
        self.listen.check().context("invalid listen config")?;

        Ok(())
    }
}

impl ImporterConfig for StatsdUdpImporterConfig {
    fn name(&self) -> &NodeName {
        &self.name
    }

    fn position(&self) -> Option<YamlDocPosition> {
        self.position.clone()
    }

    fn importer_type(&self) -> &'static str {
        IMPORTER_CONFIG_TYPE
    }

    fn diff_action(&self, new: &AnyImporterConfig) -> ImporterConfigDiffAction {
        let AnyImporterConfig::StatsDUdp(new) = new else {
            return ImporterConfigDiffAction::SpawnNew;
        };

        if self.eq(new) {
            return ImporterConfigDiffAction::NoAction;
        }

        if self.listen != new.listen {
            return ImporterConfigDiffAction::ReloadAndRespawn;
        }

        ImporterConfigDiffAction::ReloadNoRespawn
    }

    fn collector(&self) -> &NodeName {
        &self.collector
    }
}
