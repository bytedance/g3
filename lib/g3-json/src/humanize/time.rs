/*
 * SPDX-License-Identifier: Apache-2.0
 * Copyright 2023-2025 ByteDance and/or its affiliates.
 */

use std::str::FromStr;
use std::time::Duration;

use anyhow::anyhow;
use humanize_rs::ParseError;
use serde_json::Value;

pub fn as_duration(v: &Value) -> anyhow::Result<Duration> {
    match v {
        Value::String(value) => match humanize_rs::duration::parse(value) {
            Ok(v) => Ok(v),
            Err(ParseError::MissingUnit) => {
                if let Ok(u) = u64::from_str(value) {
                    Ok(Duration::from_secs(u))
                } else if let Ok(f) = f64::from_str(value) {
                    Duration::try_from_secs_f64(f).map_err(anyhow::Error::new)
                } else {
                    Err(anyhow!("unsupported duration string"))
                }
            }
            Err(e) => Err(anyhow!("invalid humanize duration string: {e}")),
        },
        Value::Number(n) => {
            if let Some(u) = n.as_u64() {
                Ok(Duration::from_secs(u))
            } else if let Some(f) = n.as_f64() {
                Duration::try_from_secs_f64(f).map_err(anyhow::Error::new)
            } else {
                Err(anyhow!("unsupported duration string"))
            }
        }
        _ => Err(anyhow!(
            "json value type for humanize duration should be 'string' or 'integer' or 'real'"
        )),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn as_duration_ok() {
        let j = json!({"v": "1h2m"});
        assert_eq!(
            as_duration(&j["v"]).unwrap(),
            Duration::from_secs(3600 + 120)
        );

        let j = json!({"v": "1000"});
        assert_eq!(as_duration(&j["v"]).unwrap(), Duration::from_secs(1000));

        let j = json!({"v": 1000});
        assert_eq!(as_duration(&j["v"]).unwrap(), Duration::from_secs(1000));

        let j = json!({"v": 1.01});
        assert_eq!(
            as_duration(&j["v"]).unwrap(),
            Duration::try_from_secs_f64(1.01).unwrap()
        );
    }

    #[test]
    fn as_duration_err() {
        let j = json!({"v": "-1000"});
        assert!(as_duration(&j["v"]).is_err());

        let j = json!({"v": "1.01"});
        assert!(as_duration(&j["v"]).is_err());

        let j = json!({"v": "-1000h"});
        assert!(as_duration(&j["v"]).is_err());

        let j = json!({"v": "1000Ah"});
        assert!(as_duration(&j["v"]).is_err());

        let j = json!({"v": "invalid"});
        assert!(as_duration(&j["v"]).is_err());

        let j = json!({"v": -1000});
        assert!(as_duration(&j["v"]).is_err());

        let j = json!({"v": f64::NAN});
        assert!(as_duration(&j["v"]).is_err());

        let j = json!({"v": ["1"]});
        assert!(as_duration(&j["v"]).is_err());
    }
}
