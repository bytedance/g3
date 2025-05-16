/*
 * SPDX-License-Identifier: Apache-2.0
 * Copyright 2024-2025 ByteDance and/or its affiliates.
 */

use std::str::FromStr;

use anyhow::anyhow;
use yaml_rust::Yaml;

use g3_types::net::TlsVersion;

pub fn as_tls_version(value: &Yaml) -> anyhow::Result<TlsVersion> {
    match value {
        Yaml::Real(s) => {
            let f = f64::from_str(s).map_err(|e| anyhow!("invalid f64 value: {e}"))?;
            TlsVersion::try_from(f)
        }
        Yaml::String(s) => TlsVersion::from_str(s),
        _ => Err(anyhow!(
            "yaml value type for tls version should be 'string' or 'float'"
        )),
    }
}
