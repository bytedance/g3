/*
 * SPDX-License-Identifier: Apache-2.0
 * Copyright 2023-2025 ByteDance and/or its affiliates.
 */

use anyhow::anyhow;
use humanize_rs::bytes::Bytes;
use serde_json::Value;

pub fn as_usize(v: &Value) -> anyhow::Result<usize> {
    match v {
        Value::String(s) => {
            let v = s.parse::<Bytes>()?;
            Ok(v.size())
        }
        Value::Number(n) => {
            if let Some(n) = n.as_u64() {
                Ok(usize::try_from(n)?)
            } else {
                Err(anyhow!("out of range json value for usize"))
            }
        }
        _ => Err(anyhow!(
            "yaml value type for humanize usize should be 'string' or 'integer'"
        )),
    }
}

pub fn as_u64(v: &Value) -> anyhow::Result<u64> {
    match v {
        Value::String(s) => {
            let v = s.parse::<Bytes<u64>>()?;
            Ok(v.size())
        }
        Value::Number(n) => n
            .as_u64()
            .ok_or_else(|| anyhow!("out of range json value for u64")),
        _ => Err(anyhow!(
            "yaml value type for humanize u64 should be 'string' or 'integer'"
        )),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn as_usize_ok() {
        let j = json!({"v": "1000"});
        assert_eq!(as_usize(&j["v"]).unwrap(), 1000);

        let j = json!({"v": "1K"});
        assert_eq!(as_usize(&j["v"]).unwrap(), 1000);

        let j = json!({"v": "1KB"});
        assert_eq!(as_usize(&j["v"]).unwrap(), 1000);

        let j = json!({"v": "1KiB"});
        assert_eq!(as_usize(&j["v"]).unwrap(), 1024);

        let j = json!({"v": 1024});
        assert_eq!(as_usize(&j["v"]).unwrap(), 1024);
    }

    #[test]
    fn as_usize_err() {
        let j = json!({"v": -1024});
        assert!(as_usize(&j["v"]).is_err());

        let j = json!({"v": 1.01});
        assert!(as_usize(&j["v"]).is_err());

        let j = json!({"v": ["1"]});
        assert!(as_usize(&j["v"]).is_err());
    }

    #[test]
    fn as_u64_ok() {
        let j = json!({"v": "1000"});
        assert_eq!(as_u64(&j["v"]).unwrap(), 1000);

        let j = json!({"v": "1K"});
        assert_eq!(as_u64(&j["v"]).unwrap(), 1000);

        let j = json!({"v": "1KB"});
        assert_eq!(as_u64(&j["v"]).unwrap(), 1000);

        let j = json!({"v": "1KiB"});
        assert_eq!(as_u64(&j["v"]).unwrap(), 1024);

        let j = json!({"v": 1024});
        assert_eq!(as_u64(&j["v"]).unwrap(), 1024);
    }

    #[test]
    fn as_u64_err() {
        let j = json!({"v": -1024});
        assert!(as_u64(&j["v"]).is_err());

        let j = json!({"v": 1.01});
        assert!(as_u64(&j["v"]).is_err());

        let j = json!({"v": ["1"]});
        assert!(as_u64(&j["v"]).is_err());
    }
}
