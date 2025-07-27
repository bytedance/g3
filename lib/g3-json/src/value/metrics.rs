/*
 * SPDX-License-Identifier: Apache-2.0
 * Copyright 2023-2025 ByteDance and/or its affiliates.
 */

use std::str::FromStr;

use anyhow::anyhow;
use serde_json::Value;

use g3_types::collection::WeightedValue;
use g3_types::metrics::NodeName;

pub fn as_metric_node_name(v: &Value) -> anyhow::Result<NodeName> {
    if let Value::String(s) = v {
        let name = NodeName::from_str(s).map_err(|e| anyhow!("invalid metric node name: {e}"))?;
        Ok(name)
    } else {
        Err(anyhow!(
            "json value type for 'metric node name' should be string"
        ))
    }
}

pub fn as_weighted_metric_node_name(value: &Value) -> anyhow::Result<WeightedValue<NodeName>> {
    if let Value::Object(map) = value {
        let mut name = NodeName::default();
        let mut weight = None;

        for (k, v) in map {
            match crate::key::normalize(k).as_str() {
                "name" => name = as_metric_node_name(v)?,
                "weight" => {
                    let f = crate::value::as_f64(v)?;
                    weight = Some(f);
                }
                _ => {}
            }
        }

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
    use serde_json::json;

    #[test]
    fn as_metric_node_name_ok() {
        // Valid node names
        let v = json!("valid-node1");
        assert_eq!(as_metric_node_name(&v).unwrap().as_str(), "valid-node1");

        let v = json!("node_with_underscore");
        assert_eq!(
            as_metric_node_name(&v).unwrap().as_str(),
            "node_with_underscore"
        );

        let v = json!("node123");
        assert_eq!(as_metric_node_name(&v).unwrap().as_str(), "node123");
    }

    #[test]
    fn as_metric_node_name_err() {
        // Empty string
        let v = json!("");
        assert!(as_metric_node_name(&v).is_err());

        // Invalid characters
        let v = json!("node@name");
        assert!(as_metric_node_name(&v).is_err());

        let v = json!("node#name");
        assert!(as_metric_node_name(&v).is_err());

        // Non-string types
        let v = json!(123);
        assert!(as_metric_node_name(&v).is_err());

        let v = json!(true);
        assert!(as_metric_node_name(&v).is_err());

        let v = json!({"name": "test"});
        assert!(as_metric_node_name(&v).is_err());
    }

    #[test]
    fn as_weighted_metric_node_name_ok() {
        // String input
        let v = json!("node1");
        let weighted = as_weighted_metric_node_name(&v).unwrap();
        assert_eq!(weighted.inner().as_str(), "node1");
        assert_eq!(weighted.weight(), 1.0);

        // Object with name and weight
        let v = json!({
            "name": "node2",
            "weight": 2.5
        });
        let weighted = as_weighted_metric_node_name(&v).unwrap();
        assert_eq!(weighted.inner().as_str(), "node2");
        assert_eq!(weighted.weight(), 2.5);

        // Object with only name
        let v = json!({
            "name": "node3"
        });
        let weighted = as_weighted_metric_node_name(&v).unwrap();
        assert_eq!(weighted.inner().as_str(), "node3");
        assert_eq!(weighted.weight(), 1.0);

        // Additional fields should be ignored
        let v = json!({
            "name": "node4",
            "weight": 1.5,
            "extra": "field"
        });
        let weighted = as_weighted_metric_node_name(&v).unwrap();
        assert_eq!(weighted.inner().as_str(), "node4");
        assert_eq!(weighted.weight(), 1.5);
    }

    #[test]
    fn as_weighted_metric_node_name_err() {
        // Empty string
        let v = json!("");
        assert!(as_weighted_metric_node_name(&v).is_err());

        // Invalid string
        let v = json!("node@name");
        assert!(as_weighted_metric_node_name(&v).is_err());

        // Object missing name
        let v = json!({
            "weight": 2.0
        });
        assert!(as_weighted_metric_node_name(&v).is_err());

        // Object with empty name
        let v = json!({
            "name": ""
        });
        assert!(as_weighted_metric_node_name(&v).is_err());

        // Object with invalid name
        let v = json!({
            "name": "node$name"
        });
        assert!(as_weighted_metric_node_name(&v).is_err());

        // Object with invalid weight type
        let v = json!({
            "name": "node5",
            "weight": "invalid"
        });
        assert!(as_weighted_metric_node_name(&v).is_err());

        // Invalid types
        let v = json!(123);
        assert!(as_weighted_metric_node_name(&v).is_err());

        let v = json!(true);
        assert!(as_weighted_metric_node_name(&v).is_err());

        let v = json!([1, 2, 3]);
        assert!(as_weighted_metric_node_name(&v).is_err());
    }
}
