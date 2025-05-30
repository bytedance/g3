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
