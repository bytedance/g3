/*
 * SPDX-License-Identifier: Apache-2.0
 * Copyright 2023-2025 ByteDance and/or its affiliates.
 */

use anyhow::{Context, anyhow};
use yaml_rust::{Yaml, yaml};

pub fn foreach_kv<F>(table: &yaml::Hash, mut f: F) -> anyhow::Result<()>
where
    F: FnMut(&str, &Yaml) -> anyhow::Result<()>,
{
    for (k, v) in table.iter() {
        if let Yaml::String(key) = k {
            f(key, v).context(format!("failed to parse value of key {key}"))?;
        } else {
            return Err(anyhow!("key in hash should be string"));
        }
    }
    Ok(())
}

pub fn get_required<'a>(map: &'a yaml::Hash, k: &str) -> anyhow::Result<&'a Yaml> {
    let key = Yaml::String(k.to_owned());
    match map.get(&key) {
        Some(v) => Ok(v),
        None => Err(anyhow!("no required key {k} found in this map")),
    }
}

pub fn get_required_str<'a>(map: &'a yaml::Hash, k: &str) -> anyhow::Result<&'a str> {
    let v = get_required(map, k)?;
    if let Yaml::String(s) = v {
        Ok(s)
    } else {
        Err(anyhow!("invalid string value for required key {k}"))
    }
}
