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
