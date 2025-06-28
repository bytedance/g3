/*
 * SPDX-License-Identifier: Apache-2.0
 * Copyright 2025 ByteDance and/or its affiliates.
 */

use std::str::FromStr;

use anyhow::anyhow;
use clap::ArgMatches;
use http::{HeaderName, HeaderValue};

pub fn get_headers(args: &ArgMatches, id: &str) -> anyhow::Result<Vec<(HeaderName, HeaderValue)>> {
    let mut headers = Vec::new();
    if let Some(v) = args.get_many::<String>(id) {
        for s in v {
            let Some((name, value)) = s.split_once(':') else {
                return Err(anyhow!("invalid HTTP header: {s}"));
            };
            let name = HeaderName::from_str(name)
                .map_err(|e| anyhow!("invalid HTTP header name {name}: {e}"))?;
            let value = HeaderValue::from_str(value)
                .map_err(|e| anyhow!("invalid HTTP header value {value}: {e}"))?;
            headers.push((name, value));
        }
    }
    Ok(headers)
}
