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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn as_metrics_name_ok() {
        // Valid ASCII string
        let v = ValueRef::from("valid_metric.name-123");
        assert_eq!(
            as_metrics_name(&v).unwrap().as_str(),
            "valid_metric.name-123"
        );

        // Valid Unicode alphanumeric
        let v = ValueRef::from("valid_metric_测试");
        assert_eq!(as_metrics_name(&v).unwrap().as_str(), "valid_metric_测试");

        // Valid boundary characters
        let v = ValueRef::from("a/b.c-d_e");
        assert_eq!(as_metrics_name(&v).unwrap().as_str(), "a/b.c-d_e");
    }

    #[test]
    fn as_metrics_name_err() {
        // Empty string
        let v = ValueRef::from("");
        assert!(as_metrics_name(&v).is_err());

        // Invalid ASCII character
        let v = ValueRef::from("invalid$char");
        assert!(as_metrics_name(&v).is_err());

        // Invalid Unicode (emoji)
        let v = ValueRef::from("invalid😊char");
        assert!(as_metrics_name(&v).is_err());

        // Non-string type
        let v = ValueRef::from(42);
        assert!(as_metrics_name(&v).is_err());
    }

    #[test]
    fn as_weighted_metrics_name_ok() {
        // Simple string input (default weight)
        let v = ValueRef::from("simple_metric");
        let result = as_weighted_metrics_name(&v).unwrap();
        assert_eq!(result.inner().as_str(), "simple_metric");
        assert_eq!(result.weight(), 1.0);

        // Map with name only
        let map = vec![(ValueRef::from("name"), ValueRef::from("map_metric"))];
        let v = ValueRef::Map(map);
        let result = as_weighted_metrics_name(&v).unwrap();
        assert_eq!(result.inner().as_str(), "map_metric");
        assert_eq!(result.weight(), 1.0);

        // Map with name and weight
        let map = vec![
            (ValueRef::from("name"), ValueRef::from("weighted_metric")),
            (ValueRef::from("weight"), ValueRef::from(2.5)),
        ];
        let v = ValueRef::Map(map);
        let result = as_weighted_metrics_name(&v).unwrap();
        assert_eq!(result.inner().as_str(), "weighted_metric");
        assert_eq!(result.weight(), 2.5);

        // Map with extra fields
        let map = vec![
            (ValueRef::from("name"), ValueRef::from("extra_field_metric")),
            (ValueRef::from("weight"), ValueRef::from(1.5)),
            (ValueRef::from("extra"), ValueRef::from("value")),
        ];
        let v = ValueRef::Map(map);
        let result = as_weighted_metrics_name(&v).unwrap();
        assert_eq!(result.inner().as_str(), "extra_field_metric");
        assert_eq!(result.weight(), 1.5);

        // Boundary weight values
        let map = vec![
            (ValueRef::from("name"), ValueRef::from("min_weight")),
            (ValueRef::from("weight"), ValueRef::from(f64::MIN_POSITIVE)),
        ];
        let v = ValueRef::Map(map);
        let result = as_weighted_metrics_name(&v).unwrap();
        assert_eq!(result.inner().as_str(), "min_weight");
        assert_eq!(result.weight(), f64::MIN_POSITIVE);

        let map = vec![
            (ValueRef::from("name"), ValueRef::from("max_weight")),
            (ValueRef::from("weight"), ValueRef::from(f64::MAX)),
        ];
        let v = ValueRef::Map(map);
        let result = as_weighted_metrics_name(&v).unwrap();
        assert_eq!(result.inner().as_str(), "max_weight");
        assert_eq!(result.weight(), f64::MAX);
    }

    #[test]
    fn as_weighted_metrics_name_err() {
        // Map missing name field
        let map = vec![(ValueRef::from("weight"), ValueRef::from(1.0))];
        let v = ValueRef::Map(map);
        assert!(as_weighted_metrics_name(&v).is_err());

        // Map with invalid name value
        let map = vec![(ValueRef::from("name"), ValueRef::from(""))];
        let v = ValueRef::Map(map);
        assert!(as_weighted_metrics_name(&v).is_err());

        // Map with non-float weight
        let map = vec![
            (ValueRef::from("name"), ValueRef::from("metric")),
            (ValueRef::from("weight"), ValueRef::from("not_a_float")),
        ];
        let v = ValueRef::Map(map);
        assert!(as_weighted_metrics_name(&v).is_err());

        // Invalid data type
        let v = ValueRef::from(123);
        assert!(as_weighted_metrics_name(&v).is_err());

        // Empty map
        let v = ValueRef::Map(vec![]);
        assert!(as_weighted_metrics_name(&v).is_err());
    }
}
