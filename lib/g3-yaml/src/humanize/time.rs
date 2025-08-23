/*
 * SPDX-License-Identifier: Apache-2.0
 * Copyright 2023-2025 ByteDance and/or its affiliates.
 */

use std::str::FromStr;
use std::time::Duration;

use anyhow::anyhow;
use humanize_rs::ParseError;
use yaml_rust::Yaml;

pub fn as_duration(v: &Yaml) -> anyhow::Result<Duration> {
    match v {
        Yaml::String(value) => match humanize_rs::duration::parse(value) {
            Ok(v) => Ok(v),
            Err(ParseError::MissingUnit) => {
                if let Ok(u) = u64::from_str(value) {
                    Ok(Duration::from_secs(u))
                } else if let Ok(f) = f64::from_str(value) {
                    Duration::try_from_secs_f64(f).map_err(anyhow::Error::new)
                } else {
                    Err(anyhow!("invalid duration string"))
                }
            }
            Err(e) => Err(anyhow!("invalid humanize duration string: {e}")),
        },
        Yaml::Integer(value) => {
            if let Ok(u) = u64::try_from(*value) {
                Ok(Duration::from_secs(u))
            } else {
                Err(anyhow!("unsupported duration string"))
            }
        }
        Yaml::Real(s) => {
            let f = f64::from_str(s).map_err(|e| anyhow!("invalid f64 value: {e}"))?;
            Duration::try_from_secs_f64(f).map_err(anyhow::Error::new)
        }
        _ => Err(anyhow!(
            "yaml value type for humanize duration should be 'string' or 'integer' or 'real'"
        )),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn as_duration_ok() {
        let v = yaml_str!("1h2m");
        assert_eq!(as_duration(&v).unwrap(), Duration::from_secs(3600 + 120));

        let v = yaml_str!("1000");
        assert_eq!(as_duration(&v).unwrap(), Duration::from_secs(1000));

        let v = Yaml::Integer(1000);
        assert_eq!(as_duration(&v).unwrap(), Duration::from_secs(1000));

        let v = Yaml::Real("1.01".to_string());
        assert_eq!(
            as_duration(&v).unwrap(),
            Duration::try_from_secs_f64(1.01).unwrap()
        );
    }

    #[test]
    fn as_duration_err() {
        let v = yaml_str!("-1000");
        assert!(as_duration(&v).is_err());

        let v = yaml_str!("1.01");
        assert!(as_duration(&v).is_err());

        let v = yaml_str!("-1000h");
        assert!(as_duration(&v).is_err());

        let v = yaml_str!("1000Ah");
        assert!(as_duration(&v).is_err());

        let v = yaml_str!("abc");
        assert!(as_duration(&v).is_err());

        let v = Yaml::Integer(-1000);
        assert!(as_duration(&v).is_err());

        let v = Yaml::Array(vec![Yaml::Integer(1)]);
        assert!(as_duration(&v).is_err());
    }
}
