/*
 * SPDX-License-Identifier: Apache-2.0
 * Copyright 2023-2025 ByteDance and/or its affiliates.
 */

use std::str::FromStr;

use anyhow::anyhow;
use clap::ArgMatches;
use humanize_rs::bytes::Bytes;

pub fn get_usize(args: &ArgMatches, id: &str) -> anyhow::Result<Option<usize>> {
    if let Some(v) = args.get_one::<String>(id) {
        if let Ok(b) = v.parse::<Bytes>() {
            Ok(Some(b.size()))
        } else if let Ok(size) = usize::from_str(v) {
            Ok(Some(size))
        } else {
            Err(anyhow!("invalid {id} value {v}"))
        }
    } else {
        Ok(None)
    }
}
