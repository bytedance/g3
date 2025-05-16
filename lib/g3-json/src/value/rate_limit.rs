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
