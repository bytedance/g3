/*
 * SPDX-License-Identifier: Apache-2.0
 * Copyright 2023-2025 ByteDance and/or its affiliates.
 */

use std::collections::BTreeSet;
use std::str::FromStr;

use anyhow::{Context, anyhow};
use serde_json::Value;

use g3_histogram::{HistogramMetricsConfig, Quantile};

pub fn as_quantile(value: &Value) -> anyhow::Result<Quantile> {
    match value {
        Value::String(s) => {
            Quantile::from_str(s).map_err(|e| anyhow!("invalid quantile value: {e}"))
        }
        Value::Number(s) => {
            if let Some(f) = s.as_f64() {
                Quantile::try_from(f).map_err(|e| anyhow!("invalid quantile value: {e}"))
            } else {
                Err(anyhow!("out of range quantile value"))
            }
        }
        _ => Err(anyhow!(
            "yaml value type for 'quantile' should be 'str' or 'float'"
        )),
    }
}

pub fn as_quantile_list(value: &Value) -> anyhow::Result<BTreeSet<Quantile>> {
    let mut set = BTreeSet::new();
    match value {
        Value::String(s) => {
            for v in s.split(',') {
                let f = Quantile::from_str(v.trim())
                    .map_err(|e| anyhow!("invalid quantile string {v}: {e}"))?;
                set.insert(f);
            }
        }
        Value::Array(seq) => {
            for (i, v) in seq.iter().enumerate() {
                let f =
                    as_quantile(v).context(format!("invalid quantile value for element #{i}"))?;
                set.insert(f);
            }
        }
        _ => {
            return Err(anyhow!(
                "the yaml value type for 'duration metrics quantile' should be 'seq' or 'str'"
            ));
        }
    }
    Ok(set)
}

pub fn as_histogram_metrics_config(value: &Value) -> anyhow::Result<HistogramMetricsConfig> {
    if let Value::Object(map) = value {
        let mut config = HistogramMetricsConfig::default();
        for (k, v) in map {
            match crate::key::normalize(k).as_str() {
                "quantile" => {
                    let quantile_list = as_quantile_list(v)
                        .context(format!("invalid quantile list value for key {k}"))?;
                    config.set_quantile_list(quantile_list);
                }
                "rotate" => {
                    let rotate = crate::humanize::as_duration(v)
                        .context(format!("invalid humanize duration value for key {k}"))?;
                    config.set_rotate_interval(rotate);
                }
                _ => return Err(anyhow!("invalid key {k}")),
            }
        }
        Ok(config)
    } else {
        let rotate = crate::humanize::as_duration(value).context(
            "the value for simplified form of histogram metrics config map should be humanize duration",
        )?;
        Ok(HistogramMetricsConfig::with_rotate(rotate))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;
    use std::time::Duration;

    #[test]
    fn as_quantile_ok() {
        // valid string inputs
        assert_eq!(as_quantile(&json!("0.5")).unwrap().value(), 0.5);
        assert_eq!(as_quantile(&json!("0.99")).unwrap().value(), 0.99);

        // valid number inputs
        assert_eq!(as_quantile(&json!(0.5)).unwrap().value(), 0.5);
        assert_eq!(as_quantile(&json!(0.99)).unwrap().value(), 0.99);

        // boundary values
        assert_eq!(as_quantile(&json!("0.0")).unwrap().value(), 0.0);
        assert_eq!(as_quantile(&json!("1.0")).unwrap().value(), 1.0);
    }

    #[test]
    fn as_quantile_err() {
        // invalid types
        assert!(as_quantile(&json!(true)).is_err());
        assert!(as_quantile(&json!([])).is_err());

        // invalid strings
        assert!(as_quantile(&json!("abc")).is_err());

        // out-of-range values
        assert!(as_quantile(&json!("-0.1")).is_err());
        assert!(as_quantile(&json!("1.1")).is_err());
    }

    #[test]
    fn as_quantile_list_ok() {
        // comma-separated string
        let list = as_quantile_list(&json!("0.5, 0.75, 0.99")).unwrap();
        assert_eq!(list.len(), 3);
        assert!(list.contains(&Quantile::from_str("0.5").unwrap()));
        assert!(list.contains(&Quantile::from_str("0.75").unwrap()));
        assert!(list.contains(&Quantile::from_str("0.99").unwrap()));

        // array format
        let list = as_quantile_list(&json!([0.5, "0.75", 0.99])).unwrap();
        assert_eq!(list.len(), 3);
        assert!(list.contains(&Quantile::from_str("0.5").unwrap()));
        assert!(list.contains(&Quantile::from_str("0.75").unwrap()));
        assert!(list.contains(&Quantile::from_str("0.99").unwrap()));

        // mixed types
        let list = as_quantile_list(&json!([0.5, "0.99"])).unwrap();
        assert_eq!(list.len(), 2);
        assert!(list.contains(&Quantile::from_str("0.5").unwrap()));
        assert!(list.contains(&Quantile::from_str("0.99").unwrap()));
    }

    #[test]
    fn as_quantile_list_err() {
        // invalid string format
        assert!(as_quantile_list(&json!("0.5;0.75")).is_err());

        // array with invalid elements
        assert!(as_quantile_list(&json!([0.5, "abc", 0.99])).is_err());

        // invalid type
        assert!(as_quantile_list(&json!(true)).is_err());
    }

    #[test]
    fn as_histogram_metrics_config_ok() {
        // simplified form (duration only)
        let config = as_histogram_metrics_config(&json!("10s")).unwrap();
        assert_eq!(config.rotate_interval(), Duration::from_secs(10));

        // full form with quantiles and rotate
        let config = as_histogram_metrics_config(&json!({
            "quantile": [0.5, 0.99],
            "rotate": "5s"
        }))
        .unwrap();
        let mut expected = HistogramMetricsConfig::default();
        let mut quantile_list = BTreeSet::new();
        quantile_list.insert(Quantile::from_str("0.5").unwrap());
        quantile_list.insert(Quantile::from_str("0.99").unwrap());
        expected.set_quantile_list(quantile_list);
        expected.set_rotate_interval(Duration::from_secs(5));
        assert_eq!(config, expected);

        // full form with quantiles as string
        let config = as_histogram_metrics_config(&json!({
            "quantile": "0.5,0.99",
            "rotate": "5s"
        }))
        .unwrap();
        assert_eq!(config, expected);
    }

    #[test]
    fn as_histogram_metrics_config_err() {
        // invalid keys
        assert!(
            as_histogram_metrics_config(&json!({
                "invalid_key": "value"
            }))
            .is_err()
        );

        // invalid duration format
        assert!(as_histogram_metrics_config(&json!("invalid")).is_err());

        // invalid quantile format
        assert!(
            as_histogram_metrics_config(&json!({
                "quantile": "invalid",
                "rotate": "5s"
            }))
            .is_err()
        );

        // invalid rotate duration
        assert!(
            as_histogram_metrics_config(&json!({
                "quantile": [0.5, 0.99],
                "rotate": "invalid"
            }))
            .is_err()
        );
    }
}
