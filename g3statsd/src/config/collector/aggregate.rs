/*
 * SPDX-License-Identifier: Apache-2.0
 * Copyright 2025 ByteDance and/or its affiliates.
 */

use std::collections::BTreeSet;
use std::time::Duration;

use anyhow::{Context, anyhow};
use yaml_rust::{Yaml, yaml};

use g3_types::metrics::{MetricTagName, NodeName};
use g3_yaml::YamlDocPosition;

use super::{AnyCollectorConfig, CollectorConfig, CollectorConfigDiffAction};

const COLLECTOR_CONFIG_TYPE: &str = "Aggregate";

#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) struct AggregateCollectorConfig {
    name: NodeName,
    position: Option<YamlDocPosition>,
    pub(crate) emit_interval: Duration,
    pub(crate) join_tags: Vec<MetricTagName>,
    pub(crate) next: Option<NodeName>,
    pub(crate) exporters: Vec<NodeName>,
}

impl AggregateCollectorConfig {
    fn new(position: Option<YamlDocPosition>) -> Self {
        AggregateCollectorConfig {
            name: NodeName::default(),
            position,
            emit_interval: Duration::from_secs(1),
            join_tags: Vec::new(),
            next: None,
            exporters: Vec::new(),
        }
    }

    pub(crate) fn parse(
        map: &yaml::Hash,
        position: Option<YamlDocPosition>,
    ) -> anyhow::Result<Self> {
        let mut collector = AggregateCollectorConfig::new(position);

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
            "emit_interval" => {
                self.emit_interval = g3_yaml::humanize::as_duration(v)
                    .context(format!("invalid humanize duration value for key {k}"))?;
                Ok(())
            }
            "join_tags" => {
                self.join_tags = g3_yaml::value::as_list(v, g3_yaml::value::as_metric_tag_name)
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

impl CollectorConfig for AggregateCollectorConfig {
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
        let AnyCollectorConfig::Aggregate(new) = new else {
            return CollectorConfigDiffAction::SpawnNew;
        };

        if self.eq(new) {
            return CollectorConfigDiffAction::NoAction;
        }

        CollectorConfigDiffAction::Update
    }

    fn dependent_collector(&self) -> Option<BTreeSet<NodeName>> {
        let next = self.next.as_ref()?;
        let mut set = BTreeSet::new();
        set.insert(next.clone());
        Some(set)
    }
}
