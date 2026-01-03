/*
 * SPDX-License-Identifier: Apache-2.0
 * Copyright 2026 G3-OSS developers.
 */

use std::str::FromStr;

use anyhow::anyhow;
use serde_json::Value;

use g3_types::auth::{FactsMatchType, FactsMatchValue};

pub fn as_facts_match_type(value: &Value) -> anyhow::Result<FactsMatchType> {
    if let Value::String(s) = value {
        Ok(FactsMatchType::from_str(s)?)
    } else {
        Err(anyhow!(
            "json value type for FactsMatchType should be string"
        ))
    }
}

pub fn as_facts_match_value(value: &Value) -> anyhow::Result<FactsMatchValue> {
    match value {
        Value::String(s) => {
            let Some((k, v)) = s.split_once(':') else {
                return Err(anyhow!(
                    "the FactsMatchValue string value should be of 'key:value' format"
                ));
            };
            FactsMatchValue::new(k.trim_end(), v.trim_start())
        }
        Value::Object(map) => {
            let Some((k, v)) = map.iter().next() else {
                return Err(anyhow!(
                    "the FactsMatchValue map value should have exactly one key"
                ));
            };
            let Value::String(v) = v else {
                return Err(anyhow!(
                    "the value in FactsMatchValue map should be a string"
                ));
            };
            FactsMatchValue::new(k, v)
        }
        _ => Err(anyhow!("invalid value type for FactsMatchValue")),
    }
}
