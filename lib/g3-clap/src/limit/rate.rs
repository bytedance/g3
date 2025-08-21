/*
 * SPDX-License-Identifier: Apache-2.0
 * Copyright 2025 ByteDance and/or its affiliates.
 */

use std::num::NonZeroU32;
use std::str::FromStr;
use std::time::Duration;

use anyhow::anyhow;
use clap::ArgMatches;

use g3_types::limit::RateLimitQuota;

pub fn get_rate_limit(args: &ArgMatches, id: &str) -> anyhow::Result<Option<RateLimitQuota>> {
    let Some(v) = args.get_one::<String>(id) else {
        return Ok(None);
    };

    let quota = if let Some((v1, v2)) = v.split_once('/') {
        let burst =
            NonZeroU32::from_str(v1.trim()).map_err(|e| anyhow!("invalid burst value: {e}"))?;
        let interval_s = v2.trim();
        if let Ok(seconds) = u64::from_str(interval_s) {
            RateLimitQuota::new(Duration::from_secs(seconds), burst)?
        } else if let Ok(interval) = humanize_rs::duration::parse(interval_s) {
            RateLimitQuota::new(interval, burst)?
        } else {
            match interval_s {
                "s" => RateLimitQuota::per_second(burst)?,
                "m" => RateLimitQuota::per_minute(burst)?,
                "h" => RateLimitQuota::per_hour(burst)?,
                _ => return Err(anyhow!("invalid interval value {v2}")),
            }
        }
    } else {
        let burst = NonZeroU32::from_str(v).map_err(|e| anyhow!("invalid burst value: {e}"))?;
        RateLimitQuota::per_second(burst)?
    };
    Ok(Some(quota))
}
