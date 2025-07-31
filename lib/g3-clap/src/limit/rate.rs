/*
 * SPDX-License-Identifier: Apache-2.0
 * Copyright 2025 ByteDance and/or its affiliates.
 */

use std::num::NonZeroU32;
use std::str::FromStr;
use std::time::Duration;

use anyhow::anyhow;
use clap::ArgMatches;
use governor::Quota;

pub fn get_rate_limit(args: &ArgMatches, id: &str) -> anyhow::Result<Option<Quota>> {
    if let Some(v) = args.get_one::<String>(id) {
        match v.split_once('/') {
            Some((v1, v2)) => {
                let burst = NonZeroU32::from_str(v1.trim())
                    .map_err(|e| anyhow!("invalid burst value: {e}"))?;
                let interval_s = v2.trim();
                if let Ok(seconds) = u64::from_str(interval_s) {
                    let quota = if burst.get() > 1000000 {
                        let replenish_nanos = seconds * 1000000000 / (burst.get() as u64);
                        Quota::with_period(Duration::from_nanos(replenish_nanos))
                    } else {
                        let replenish_micros = seconds * 1000000 / (burst.get() as u64);
                        Quota::with_period(Duration::from_micros(replenish_micros))
                    };
                    return Ok(quota);
                }
                if let Ok(interval) = humanize_rs::duration::parse(interval_s) {
                    let quota = if burst.get() > 1000000 {
                        let nanos = interval.as_nanos() as u64;
                        let replenish_nanos = nanos / (burst.get() as u64);
                        Quota::with_period(Duration::from_nanos(replenish_nanos))
                    } else {
                        let micros = interval.as_micros() as u64;
                        let replenish_micros = micros / (burst.get() as u64);
                        Quota::with_period(Duration::from_micros(replenish_micros))
                    };
                    return Ok(quota);
                }
                match interval_s {
                    "s" => Ok(Some(Quota::per_second(burst))),
                    "m" => Ok(Some(Quota::per_minute(burst))),
                    "h" => Ok(Some(Quota::per_hour(burst))),
                    _ => Err(anyhow!("invalid interval value {v}")),
                }
            }
            None => {
                let burst =
                    NonZeroU32::from_str(v).map_err(|e| anyhow!("invalid burst value: {e}"))?;
                Ok(Some(Quota::per_second(burst)))
            }
        }
    } else {
        Ok(None)
    }
}
