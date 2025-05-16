/*
 * SPDX-License-Identifier: Apache-2.0
 * Copyright 2023-2025 ByteDance and/or its affiliates.
 */

use std::collections::BTreeSet;
use std::str::FromStr;

use anyhow::{Context, anyhow};
use yaml_rust::Yaml;

use g3_histogram::{HistogramMetricsConfig, Quantile};

pub fn as_quantile(value: &Yaml) -> anyhow::Result<Quantile> {
    match value {
        Yaml::String(s) => {
            Quantile::from_str(s).map_err(|e| anyhow!("invalid quantile value: {e}"))
        }
        Yaml::Real(s) => Quantile::from_str(s).map_err(|e| anyhow!("invalid quantile value: {e}")),
        _ => Err(anyhow!(
            "yaml value type for 'quantile' should be 'str' or 'float'"
        )),
    }
}

pub fn as_quantile_list(value: &Yaml) -> anyhow::Result<BTreeSet<Quantile>> {
    let mut set = BTreeSet::new();
    match value {
        Yaml::String(s) => {
            for v in s.split(',') {
                let f = Quantile::from_str(v.trim())
                    .map_err(|e| anyhow!("invalid quantile string {v}: {e}"))?;
                set.insert(f);
            }
        }
        Yaml::Array(seq) => {
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

pub fn as_histogram_metrics_config(value: &Yaml) -> anyhow::Result<HistogramMetricsConfig> {
    if let Yaml::Hash(map) = value {
        let mut config = HistogramMetricsConfig::default();
        crate::foreach_kv(map, |k, v| match crate::key::normalize(k).as_str() {
            "quantile" => {
                let quantile_list = as_quantile_list(v)
                    .context(format!("invalid quantile list value for key {k}"))?;
                config.set_quantile_list(quantile_list);
                Ok(())
            }
            "rotate" => {
                let rotate = crate::humanize::as_duration(v)
                    .context(format!("invalid humanize duration value for key {k}"))?;
                config.set_rotate_interval(rotate);
                Ok(())
            }
            _ => Err(anyhow!("invalid key {k}")),
        })?;
        Ok(config)
    } else {
        let rotate = crate::humanize::as_duration(value).context(
            "the value for simplified form of histogram metrics config map should be humanize duration",
        )?;
        Ok(HistogramMetricsConfig::with_rotate(rotate))
    }
}
