/*
 * SPDX-License-Identifier: Apache-2.0
 * Copyright 2023-2025 ByteDance and/or its affiliates.
 */

use std::str::FromStr;

use anyhow::{Context, anyhow};
use rmpv::ValueRef;

use g3_types::collection::WeightedValue;
use g3_types::metrics::NodeName;

pub fn as_metrics_name(v: &ValueRef) -> anyhow::Result<NodeName> {
    if let ValueRef::String(s) = v {
        let s = s.as_str().ok_or_else(|| anyhow!("invalid utf-8 string"))?;
        let name = NodeName::from_str(s).map_err(|e| anyhow!("invalid metrics name: {e}"))?;
        Ok(name)
    } else {
        Err(anyhow!(
            "msgpack value type for 'metrics name' should be 'string'"
        ))
    }
}

pub fn as_weighted_metrics_name(v: &ValueRef) -> anyhow::Result<WeightedValue<NodeName>> {
    match v {
        ValueRef::Map(map) => {
            let mut name = NodeName::default();
            let mut weight = None;

            for (k, v) in map {
                let key = as_metrics_name(k).context("all keys should be metrics name")?;
                match crate::key::normalize(key.as_str()).as_str() {
                    "name" => {
                        name = as_metrics_name(v)
                            .context(format!("invalid metrics name value for key {key}"))?;
                    }
                    "weight" => {
                        let f = crate::value::as_f64(v)
                            .context(format!("invalid f64 value for key {key}"))?;
                        weight = Some(f);
                    }
                    _ => {} // ignore all other keys
                }
            }

            if name.is_empty() {
                Err(anyhow!("no name found"))
            } else if let Some(weight) = weight {
                Ok(WeightedValue::with_weight(name, weight))
            } else {
                Ok(WeightedValue::new(name))
            }
        }
        _ => {
            let s = as_metrics_name(v).context("invalid string value")?;
            Ok(WeightedValue::new(s))
        }
    }
}
