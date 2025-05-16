/*
 * SPDX-License-Identifier: Apache-2.0
 * Copyright 2023-2025 ByteDance and/or its affiliates.
 */

use std::str::FromStr;
use std::time::Duration;

use anyhow::anyhow;
use clap::ArgMatches;

pub fn get_duration(args: &ArgMatches, id: &str) -> anyhow::Result<Option<Duration>> {
    if let Some(v) = args.get_one::<String>(id) {
        if let Ok(timeout) = humanize_rs::duration::parse(v) {
            Ok(Some(timeout))
        } else if let Ok(timeout) = u64::from_str(v) {
            Ok(Some(Duration::from_secs(timeout)))
        } else if let Ok(timeout) = f64::from_str(v) {
            let timeout = Duration::try_from_secs_f64(timeout)
                .map_err(|e| anyhow!("out of range timeout value: {e}"))?;
            Ok(Some(timeout))
        } else {
            Err(anyhow!("invalid {id} value {v}"))
        }
    } else {
        Ok(None)
    }
}
