/*
 * SPDX-License-Identifier: Apache-2.0
 * Copyright 2023-2025 ByteDance and/or its affiliates.
 */

use std::num::NonZeroU32;
use std::str::FromStr;

use anyhow::{Context, anyhow};
use serde_json::Value;

use g3_types::limit::RateLimitQuotaConfig;

pub fn as_rate_limit_quota(v: &Value) -> anyhow::Result<RateLimitQuotaConfig> {
    match v {
        Value::Number(_) => {
            let count = crate::value::as_nonzero_u32(v)?;
            Ok(RateLimitQuotaConfig::per_second(count))
        }
        Value::String(s) => RateLimitQuotaConfig::from_str(s),
        Value::Object(map) => {
            let mut quota: Option<RateLimitQuotaConfig> = None;
            let mut max_burst: Option<NonZeroU32> = None;
            for (k, v) in map {
                match crate::key::normalize(k).as_str() {
                    "rate" => match v {
                        Value::Number(_) | Value::String(_) => {
                            quota = Some(
                                as_rate_limit_quota(v)
                                    .context(format!("invalid value for key {k}"))?,
                            );
                        }
                        _ => return Err(anyhow!("invalid value type for key {k}")),
                    },
                    "replenish_interval" => {
                        let dur = crate::humanize::as_duration(v)
                            .context(format!("invalid humanize duration value for key {k}"))?;
                        quota = RateLimitQuotaConfig::with_period(dur);
                    }
                    "max_burst" => {
                        max_burst = Some(
                            crate::value::as_nonzero_u32(v)
                                .context(format!("invalid nonzero u32 value for key {k}"))?,
                        );
                    }
                    _ => return Err(anyhow!("invalid key {k}")),
                }
            }

            match quota {
                Some(mut quota) => {
                    if let Some(max_burst) = max_burst {
                        quota.allow_burst(max_burst);
                    }
                    Ok(quota)
                }
                None => Err(anyhow!("no rate / replenish_interval is set")),
            }
        }
        _ => Err(anyhow!("invalid json value type")),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;
    use std::time::Duration;

    #[test]
    fn as_rate_limit_quota_ok() {
        // number input
        let v = json!(10);
        assert_eq!(
            as_rate_limit_quota(&v).unwrap(),
            RateLimitQuotaConfig::per_second(NonZeroU32::new(10).unwrap())
        );

        // string input: simple number
        let v = json!("10");
        assert_eq!(
            as_rate_limit_quota(&v).unwrap(),
            RateLimitQuotaConfig::per_second(NonZeroU32::new(10).unwrap())
        );

        // string input: with unit
        let v = json!("10/s");
        assert_eq!(
            as_rate_limit_quota(&v).unwrap(),
            RateLimitQuotaConfig::per_second(NonZeroU32::new(10).unwrap())
        );

        // object input with rate and max_burst
        let v = json!({
            "rate": 10,
            "max_burst": 30
        });
        let mut expected = RateLimitQuotaConfig::per_second(NonZeroU32::new(10).unwrap());
        expected.allow_burst(NonZeroU32::new(30).unwrap());
        assert_eq!(as_rate_limit_quota(&v).unwrap(), expected);

        // object input with replenish_interval and max_burst
        let v = json!({
            "replenish_interval": "100ms",
            "max_burst": 30
        });
        let mut expected = RateLimitQuotaConfig::with_period(Duration::from_millis(100)).unwrap();
        expected.allow_burst(NonZeroU32::new(30).unwrap());
        assert_eq!(as_rate_limit_quota(&v).unwrap(), expected);

        // different string formats
        let v = json!("10/m");
        assert_eq!(
            as_rate_limit_quota(&v).unwrap(),
            RateLimitQuotaConfig::from_str("10/m").unwrap()
        );

        let v = json!("10/h");
        assert_eq!(
            as_rate_limit_quota(&v).unwrap(),
            RateLimitQuotaConfig::from_str("10/h").unwrap()
        );
    }

    #[test]
    fn as_rate_limit_quota_err() {
        // Invalid type: boolean
        let v = json!(true);
        assert!(as_rate_limit_quota(&v).is_err());

        // Invalid key in object
        let v = json!({
            "invalid_key": 10
        });
        assert!(as_rate_limit_quota(&v).is_err());

        // Missing rate/replenish_interval in object
        let v = json!({
            "max_burst": 30
        });
        assert!(as_rate_limit_quota(&v).is_err());

        // Invalid type for rate field
        let v = json!({
            "rate": [],
            "max_burst": 30
        });
        assert!(as_rate_limit_quota(&v).is_err());

        // Invalid type for max_burst field
        let v = json!({
            "rate": 10,
            "max_burst": "invalid"
        });
        assert!(as_rate_limit_quota(&v).is_err());

        // Invalid string format
        let v = json!("10invalid");
        assert!(as_rate_limit_quota(&v).is_err());

        // Invalid duration format
        let v = json!({
            "replenish_interval": "invalid",
            "max_burst": 30
        });
        assert!(as_rate_limit_quota(&v).is_err());

        // Empty object
        let v = json!({});
        assert!(as_rate_limit_quota(&v).is_err());
    }
}
