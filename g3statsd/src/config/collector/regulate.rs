/*
 * SPDX-License-Identifier: Apache-2.0
 * Copyright 2025 ByteDance and/or its affiliates.
 */

use std::collections::BTreeSet;

use anyhow::{Context, anyhow};
use yaml_rust::{Yaml, yaml};

use g3_types::metrics::{MetricTagName, NodeName};
use g3_yaml::YamlDocPosition;

use super::{AnyCollectorConfig, CollectorConfig, CollectorConfigDiffAction};
use crate::types::MetricName;

const COLLECTOR_CONFIG_TYPE: &str = "Regulate";

#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) struct RegulateCollectorConfig {
    name: NodeName,
    position: Option<YamlDocPosition>,
    pub(crate) prefix: Option<MetricName>,
    pub(crate) drop_tags: Vec<MetricTagName>,
    pub(crate) next: Option<NodeName>,
    pub(crate) exporters: Vec<NodeName>,
}

impl RegulateCollectorConfig {
    fn new(position: Option<YamlDocPosition>) -> Self {
        RegulateCollectorConfig {
            name: NodeName::default(),
            position,
            prefix: None,
            drop_tags: Vec::new(),
            next: None,
            exporters: Vec::new(),
        }
    }

    pub(crate) fn parse(
        map: &yaml::Hash,
        position: Option<YamlDocPosition>,
    ) -> anyhow::Result<Self> {
        let mut collector = RegulateCollectorConfig::new(position);

        g3_yaml::foreach_kv(map, |k, v| collector.set(k, v))?;

        collector.check()?;
        Ok(collector)
    }

    fn set(&mut self, k: &str, v: &Yaml) -> anyhow::Result<()> {
        match g3_yaml::key::normalize(k).as_str() {
            super::CONFIG_KEY_COLLECTOR_TYPE => Ok(()),
            super::CONFIG_KEY_COLLECTOR_NAME => {
                self.name = g3_yaml::value::as_metric_node_name(v)?;
                Ok(())
            }
            "prefix" => {
                let prefix = MetricName::parse_yaml(v)
                    .context(format!("invalid metric name value for key {k}"))?;
                self.prefix = Some(prefix);
                Ok(())
            }
            "drop_tags" => {
                self.drop_tags = g3_yaml::value::as_list(v, g3_yaml::value::as_metric_tag_name)
                    .context(format!("invalid list of metric tag names for key {k}"))?;
                Ok(())
            }
            "next" => {
                let next = g3_yaml::value::as_metric_node_name(v)?;
                self.next = Some(next);
                Ok(())
            }
            "exporter" => {
                self.exporters = g3_yaml::value::as_list(v, g3_yaml::value::as_metric_node_name)
                    .context(format!("invalid list of exporter names for key {k}"))?;
                Ok(())
            }
            _ => Err(anyhow!("invalid key {k}")),
        }
    }

    fn check(&mut self) -> anyhow::Result<()> {
        if self.name.is_empty() {
            return Err(anyhow!("name is not set"));
        }
        Ok(())
    }
}

impl CollectorConfig for RegulateCollectorConfig {
    fn name(&self) -> &NodeName {
        &self.name
    }

    fn position(&self) -> Option<YamlDocPosition> {
        self.position.clone()
    }

    fn collector_type(&self) -> &'static str {
        COLLECTOR_CONFIG_TYPE
    }

    fn diff_action(&self, new: &AnyCollectorConfig) -> CollectorConfigDiffAction {
        let AnyCollectorConfig::Regulate(new) = new else {
            return CollectorConfigDiffAction::SpawnNew;
        };

        if self.eq(new) {
            return CollectorConfigDiffAction::NoAction;
        }

        CollectorConfigDiffAction::Reload
    }

    fn dependent_collector(&self) -> Option<BTreeSet<NodeName>> {
        let next = self.next.as_ref()?;
        let mut set = BTreeSet::new();
        set.insert(next.clone());
        Some(set)
    }
}
