/*
 * SPDX-License-Identifier: Apache-2.0
 * Copyright 2024-2025 ByteDance and/or its affiliates.
 */

use std::str::FromStr;

use anyhow::anyhow;
use serde_json::Value;

use g3_types::net::TlsVersion;

pub fn as_tls_version(value: &Value) -> anyhow::Result<TlsVersion> {
    match value {
        Value::String(s) => TlsVersion::from_str(s),
        Value::Number(n) => {
            let Some(f) = n.as_f64() else {
                return Err(anyhow!("invalid f64 number value"));
            };
            TlsVersion::try_from(f)
        }
        _ => Err(anyhow!(
            "json value type for tls version should be 'string' or 'float'"
        )),
    }
}
