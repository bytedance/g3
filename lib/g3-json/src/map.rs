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

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn get_required_str_ok() {
        let mut map = Map::new();
        map.insert("valid_key".to_string(), json!("test_value"));
        assert_eq!(get_required_str(&map, "valid_key").unwrap(), "test_value");

        map.insert("empty_key".to_string(), json!(""));
        assert_eq!(get_required_str(&map, "empty_key").unwrap(), "");
    }

    #[test]
    fn get_required_str_err() {
        let mut map = Map::new();
        assert!(get_required_str(&map, "missing_key").is_err());

        map.insert("invalid_type_key".to_string(), json!(42));
        assert!(get_required_str(&map, "invalid_type_key").is_err());
    }

    #[test]
    fn get_required_ok() {
        let mut map = Map::new();
        map.insert("string_key".to_string(), json!("value"));
        map.insert("number_key".to_string(), json!(123));
        map.insert("object_key".to_string(), json!({"field": "data"}));
        assert!(get_required(&map, "string_key").is_ok());
        assert!(get_required(&map, "number_key").is_ok());
        assert!(get_required(&map, "object_key").is_ok());
    }

    #[test]
    fn get_required_err() {
        let map = Map::new();
        assert!(get_required(&map, "non_existent_key").is_err());
    }
}
