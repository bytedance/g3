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

#[cfg(test)]
mod tests {
    use super::*;
    use std::time::Duration;
    use yaml_rust::YamlLoader;

    #[test]
    fn as_quantile_ok() {
        // valid string quantiles
        assert_eq!(as_quantile(&yaml_str!("0.5")).unwrap().value(), 0.5);
        assert_eq!(as_quantile(&yaml_str!("0.99")).unwrap().value(), 0.99);

        // valid float quantiles
        assert_eq!(as_quantile(&Yaml::Real("0.5".into())).unwrap().value(), 0.5);
        assert_eq!(
            as_quantile(&Yaml::Real("0.99".into())).unwrap().value(),
            0.99
        );
    }

    #[test]
    fn as_quantile_err() {
        // invalid types
        assert!(as_quantile(&Yaml::Integer(1)).is_err());
        assert!(as_quantile(&Yaml::Boolean(true)).is_err());

        // out-of-range values
        assert!(as_quantile(&yaml_str!("-0.1")).is_err());
        assert!(as_quantile(&yaml_str!("1.1")).is_err());

        // malformed strings
        assert!(as_quantile(&yaml_str!("abc")).is_err());
    }

    #[test]
    fn as_quantile_list_ok() {
        // comma-separated string
        let list = as_quantile_list(&yaml_str!("0.5, 0.75, 0.99")).unwrap();
        assert_eq!(list.len(), 3);
        assert!(list.contains(&Quantile::from_str("0.5").unwrap()));
        assert!(list.contains(&Quantile::from_str("0.75").unwrap()));
        assert!(list.contains(&Quantile::from_str("0.99").unwrap()));

        // array format
        let yaml = yaml_doc!("- 0.5\n- 0.75\n- 0.99");
        let list = as_quantile_list(&yaml).unwrap();
        assert_eq!(list.len(), 3);
        assert!(list.contains(&Quantile::from_str("0.5").unwrap()));
        assert!(list.contains(&Quantile::from_str("0.75").unwrap()));
        assert!(list.contains(&Quantile::from_str("0.99").unwrap()));
    }

    #[test]
    fn as_quantile_list_err() {
        // invalid string format
        assert!(as_quantile_list(&yaml_str!("0.5;0.75")).is_err());

        // array with invalid elements
        let yaml = yaml_doc!("- 0.5\n- invalid\n- 0.99");
        assert!(as_quantile_list(&yaml).is_err());

        // invalid type
        assert!(as_quantile_list(&Yaml::Boolean(true)).is_err());
    }

    #[test]
    fn as_histogram_metrics_config_ok() {
        // simplified form (duration only)
        let config = as_histogram_metrics_config(&yaml_str!("10s")).unwrap();
        let expected = HistogramMetricsConfig::with_rotate(Duration::from_secs(10));
        assert_eq!(config.rotate_interval(), expected.rotate_interval());

        // full form with quantiles
        let yaml = yaml_doc!("quantile: 0.5,0.99\nrotate: 5s");
        let config = as_histogram_metrics_config(&yaml).unwrap();
        let mut expected = HistogramMetricsConfig::default();
        let mut quantile_list = BTreeSet::new();
        quantile_list.insert(Quantile::from_str("0.5").unwrap());
        quantile_list.insert(Quantile::from_str("0.99").unwrap());
        expected.set_quantile_list(quantile_list);
        expected.set_rotate_interval(Duration::from_secs(5));
        assert_eq!(config, expected);

        // other valid formats
        let yaml = Yaml::Integer(10);
        let config = as_histogram_metrics_config(&yaml).unwrap();
        let expected = HistogramMetricsConfig::with_rotate(Duration::from_secs(10));
        assert_eq!(config.rotate_interval(), expected.rotate_interval());
    }

    #[test]
    fn as_histogram_metrics_config_err() {
        // invalid keys
        let yaml = yaml_doc!("invalid_key: value");
        assert!(as_histogram_metrics_config(&yaml).is_err());

        // invalid duration format
        assert!(as_histogram_metrics_config(&yaml_str!("invalid")).is_err());

        // invalid quantile format
        let yaml = yaml_doc!("quantile: invalid\nrotate: 5s");
        assert!(as_histogram_metrics_config(&yaml).is_err());

        // non-hash input
        let yaml = Yaml::Null;
        assert!(as_histogram_metrics_config(&yaml).is_err());
    }
}
