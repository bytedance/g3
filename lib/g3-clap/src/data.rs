/*
 * SPDX-License-Identifier: Apache-2.0
 * Copyright 2025 ByteDance and/or its affiliates.
 */

use anyhow::anyhow;
use clap::ArgMatches;

pub fn get(args: &ArgMatches, id: &str, decode_binary: bool) -> anyhow::Result<Vec<u8>> {
    let Some(s) = args.get_one::<String>(id) else {
        return Ok(Vec::new());
    };

    let raw = if let Some(p) = s.strip_prefix('@') {
        std::fs::read(p).map_err(|e| anyhow!("failed to read content from file {p}: {e}"))?
    } else if let Some(name) = s.strip_prefix('$') {
        std::env::var(name)
            .map(|s| s.into_bytes())
            .map_err(|e| anyhow!("failed to read environment variable {name}: {e}"))?
    } else {
        s.clone().into_bytes()
    };

    if decode_binary {
        hex::decode(raw).map_err(|e| anyhow!("not valid hex encoded request struct: {e}"))
    } else {
        Ok(raw)
    }
}
