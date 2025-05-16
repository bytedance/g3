/*
 * SPDX-License-Identifier: Apache-2.0
 * Copyright 2023-2025 ByteDance and/or its affiliates.
 */

use std::str::FromStr;

use anyhow::{Context, anyhow};
use yaml_rust::Yaml;

use g3_types::collection::WeightedValue;
use g3_types::metrics::{MetricTagMap, MetricTagName, MetricTagValue, NodeName};

pub fn as_metric_node_name(v: &Yaml) -> anyhow::Result<NodeName> {
    if let Yaml::String(s) = v {
        let name = NodeName::from_str(s).map_err(|e| anyhow!("invalid metric node name: {e}"))?;
        Ok(name)
    } else {
        Err(anyhow!(
            "yaml value type for 'metric node name' should be 'string'"
        ))
    }
}

pub fn as_metric_tag_name(v: &Yaml) -> anyhow::Result<MetricTagName> {
    if let Yaml::String(s) = v {
        let name =
            MetricTagName::from_str(s).map_err(|e| anyhow!("invalid metric tag name: {e}"))?;
        Ok(name)
    } else {
        Err(anyhow!(
            "yaml value type for 'metric tag name' should be 'string'"
        ))
    }
}

pub fn as_metric_tag_value(v: &Yaml) -> anyhow::Result<MetricTagValue> {
    let s = crate::value::as_string(v).context("invalid yaml value for metric tag value")?;
    MetricTagValue::from_str(&s).map_err(|e| anyhow!("invalid metric tag value string {s}: {e}"))
}

pub fn as_static_metrics_tags(v: &Yaml) -> anyhow::Result<MetricTagMap> {
    if let Yaml::Hash(map) = v {
        let mut tags = MetricTagMap::default();
        crate::foreach_kv(map, |k, v| {
            let name = MetricTagName::from_str(k).context("invalid metrics tag name")?;
            let value = as_metric_tag_value(v)?;

            if tags.insert(name, value).is_some() {
                Err(anyhow!("found duplicate value for tag name {k}"))
            } else {
                Ok(())
            }
        })?;
        Ok(tags)
    } else {
        Err(anyhow!(
            "the yaml value type for 'static metric tags' should be 'map'"
        ))
    }
}

pub fn as_weighted_metric_node_name(value: &Yaml) -> anyhow::Result<WeightedValue<NodeName>> {
    if let Yaml::Hash(map) = value {
        let mut name = NodeName::default();
        let mut weight = None;

        crate::foreach_kv(map, |k, v| match crate::key::normalize(k).as_str() {
            "name" => {
                name = as_metric_node_name(v)?;
                Ok(())
            }
            "weight" => {
                let f = crate::value::as_f64(v)?;
                weight = Some(f);
                Ok(())
            }
            _ => Ok(()),
        })?;

        if name.is_empty() {
            Err(anyhow!("no name found"))
        } else if let Some(weight) = weight {
            Ok(WeightedValue::with_weight(name, weight))
        } else {
            Ok(WeightedValue::new(name))
        }
    } else {
        let name = as_metric_node_name(value)?;
        Ok(WeightedValue::new(name))
    }
}
