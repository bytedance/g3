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

#[cfg(test)]
mod tests {
    use super::*;
    use yaml_rust::YamlLoader;

    #[test]
    fn as_metric_node_name_ok() {
        // Valid node names
        let yaml = yaml_str!("valid-node1");
        assert_eq!(as_metric_node_name(&yaml).unwrap().as_str(), "valid-node1");

        let yaml = yaml_str!("node_with_underscore");
        assert_eq!(
            as_metric_node_name(&yaml).unwrap().as_str(),
            "node_with_underscore"
        );
    }

    #[test]
    fn as_metric_node_name_err() {
        // Empty string
        let yaml = yaml_str!("");
        assert!(as_metric_node_name(&yaml).is_err());

        // Invalid characters
        let yaml = yaml_str!("node@name");
        assert!(as_metric_node_name(&yaml).is_err());

        // Non-string type
        let yaml = Yaml::Integer(123);
        assert!(as_metric_node_name(&yaml).is_err());
    }

    #[test]
    fn as_metric_tag_name_ok() {
        // Valid tag names
        let yaml = yaml_str!("valid_tag");
        assert_eq!(as_metric_tag_name(&yaml).unwrap().as_str(), "valid_tag");

        let yaml = yaml_str!("tag_with_numbers123");
        assert_eq!(
            as_metric_tag_name(&yaml).unwrap().as_str(),
            "tag_with_numbers123"
        );
    }

    #[test]
    fn as_metric_tag_name_err() {
        // Invalid characters
        let yaml = yaml_str!("tag=invalid");
        assert!(as_metric_tag_name(&yaml).is_err());

        // Non-string type
        let yaml = Yaml::Boolean(true);
        assert!(as_metric_tag_name(&yaml).is_err());
    }

    #[test]
    fn as_metric_tag_value_ok() {
        // Valid tag values
        let yaml = yaml_str!("valid_value");
        assert_eq!(as_metric_tag_value(&yaml).unwrap().as_str(), "valid_value");

        let yaml = yaml_str!("value_with_underscores");
        assert_eq!(
            as_metric_tag_value(&yaml).unwrap().as_str(),
            "value_with_underscores"
        );
    }

    #[test]
    fn as_metric_tag_value_err() {
        // Invalid characters
        let yaml = yaml_str!("value=invalid");
        assert!(as_metric_tag_value(&yaml).is_err());

        // Non-string type
        let yaml = Yaml::Array(vec![]);
        assert!(as_metric_tag_value(&yaml).is_err());
    }

    #[test]
    fn as_static_metrics_tags_ok() {
        // Empty map
        let yaml = yaml_doc!("{}");
        assert!(as_static_metrics_tags(&yaml).unwrap().is_empty());

        // Single key-value pair
        let yaml = yaml_doc!(
            r#"
                key1: value1
            "#
        );
        let tags = as_static_metrics_tags(&yaml).unwrap();
        assert_eq!(tags.len(), 1);
        assert_eq!(
            tags.get(&MetricTagName::from_str("key1").unwrap())
                .unwrap()
                .as_str(),
            "value1"
        );

        // Multiple key-value pairs
        let yaml = yaml_doc!(
            r#"
                key1: value1
                key2: value2
            "#
        );
        let tags = as_static_metrics_tags(&yaml).unwrap();
        assert_eq!(tags.len(), 2);
        assert_eq!(
            tags.get(&MetricTagName::from_str("key1").unwrap())
                .unwrap()
                .as_str(),
            "value1"
        );
        assert_eq!(
            tags.get(&MetricTagName::from_str("key2").unwrap())
                .unwrap()
                .as_str(),
            "value2"
        );
    }

    #[test]
    fn as_static_metrics_tags_err() {
        // Non-map type
        let yaml = yaml_str!("invalid");
        assert!(as_static_metrics_tags(&yaml).is_err());

        // Invalid key
        let yaml = yaml_doc!(
            r#"
                invalid@key: value
            "#
        );
        assert!(as_static_metrics_tags(&yaml).is_err());

        // Invalid value
        let yaml = yaml_doc!(
            r#"
                key: invalid@value
            "#
        );
        assert!(as_static_metrics_tags(&yaml).is_err());
    }

    #[test]
    fn as_weighted_metric_node_name_ok() {
        // String input
        let yaml = yaml_str!("node1");
        let weighted = as_weighted_metric_node_name(&yaml).unwrap();
        assert_eq!(weighted.inner().as_str(), "node1");
        assert_eq!(weighted.weight(), 1.0);

        // Map with name and weight
        let yaml = yaml_doc!(
            r#"
                name: node2
                weight: 2.5
            "#
        );
        let weighted = as_weighted_metric_node_name(&yaml).unwrap();
        assert_eq!(weighted.inner().as_str(), "node2");
        assert_eq!(weighted.weight(), 2.5);

        // Map with only name
        let yaml = yaml_doc!(
            r#"
                name: node3
            "#
        );
        let weighted = as_weighted_metric_node_name(&yaml).unwrap();
        assert_eq!(weighted.inner().as_str(), "node3");
        assert_eq!(weighted.weight(), 1.0);
    }

    #[test]
    fn as_weighted_metric_node_name_err() {
        // Empty string
        let yaml = yaml_str!("");
        assert!(as_weighted_metric_node_name(&yaml).is_err());

        // Map with no name
        let yaml = yaml_doc!(
            r#"
                weight: 2.0
            "#
        );
        assert!(as_weighted_metric_node_name(&yaml).is_err());

        // Invalid name in map
        let yaml = yaml_doc!(
            r#"
                name: invalid@node
            "#
        );
        assert!(as_weighted_metric_node_name(&yaml).is_err());

        // Invalid weight type
        let yaml = yaml_doc!(
            r#"
                name: node4
                weight: invalid
            "#
        );
        assert!(as_weighted_metric_node_name(&yaml).is_err());
    }
}
