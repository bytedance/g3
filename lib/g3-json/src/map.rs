/*
 * SPDX-License-Identifier: Apache-2.0
 * Copyright 2023-2025 ByteDance and/or its affiliates.
 */

use anyhow::anyhow;
use serde_json::{Map, Value};

pub fn get_required_str<'a>(map: &'a Map<String, Value>, k: &str) -> anyhow::Result<&'a str> {
    match map.get(k) {
        Some(v) => match v {
            Value::String(s) => Ok(s),
            _ => Err(anyhow!("invalid string value for key {k}")),
        },
        None => Err(anyhow!("no key {k} found in this map")),
    }
}

pub fn get_required<'a>(map: &'a Map<String, Value>, k: &str) -> anyhow::Result<&'a Value> {
    match map.get(k) {
        Some(v) => Ok(v),
        None => Err(anyhow!("no key {k} found in this map")),
    }
}
