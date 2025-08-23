/*
 * SPDX-License-Identifier: Apache-2.0
 * Copyright 2023-2025 ByteDance and/or its affiliates.
 */

use std::str::FromStr;

use anyhow::anyhow;
use rand::distr::Bernoulli;
use serde_json::Value;

pub fn as_random_ratio(value: &Value) -> anyhow::Result<Bernoulli> {
    match value {
        Value::Number(n) => {
            if let Some(f) = n.as_f64() {
                Bernoulli::new(f).map_err(|e| anyhow!("out of range f64 ratio: {e}"))
            } else {
                Err(anyhow!("invalid f64 ration value"))
            }
        }
        Value::String(s) => {
            if let Some(p) = s.find('/') {
                let n1 = u32::from_str(s[0..p].trim())
                    .map_err(|e| anyhow!("first part is not valid u32: {e}"))?;
                let n2 = u32::from_str(s[p + 1..].trim())
                    .map_err(|e| anyhow!("second part is not valid u32: {e}"))?;
                Bernoulli::from_ratio(n1, n2)
                    .map_err(|e| anyhow!("out of range fraction ratio: {e}"))
            } else if let Some(s) = s.strip_suffix('%') {
                let n = u32::from_str(s.trim())
                    .map_err(|e| anyhow!("the part before % is not valid u32: {e}"))?;
                Bernoulli::from_ratio(n, 100)
                    .map_err(|e| anyhow!("out of range percentage ratio: {e}"))
            } else {
                let f = f64::from_str(s).map_err(|e| anyhow!("invalid f64 ratio string: {e}"))?;
                Bernoulli::new(f).map_err(|e| anyhow!("out of range f64 ratio: {e}"))
            }
        }
        Value::Bool(true) => Ok(Bernoulli::new(1.0).unwrap()),
        Value::Bool(false) => Ok(Bernoulli::new(0.0).unwrap()),
        _ => Err(anyhow!(
            "yaml value type for 'random ratio' should be 'f64' or 'string'"
        )),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn as_random_ratio_ok() {
        // valid Number values
        assert_eq!(as_random_ratio(&json!(0.5)).unwrap().p(), 0.5);
        assert_eq!(as_random_ratio(&json!(1.0)).unwrap().p(), 1.0);
        assert_eq!(as_random_ratio(&json!(0.0)).unwrap().p(), 0.0);

        // valid fraction format strings
        assert_eq!(as_random_ratio(&json!("1/2")).unwrap().p(), 0.5);
        assert_eq!(as_random_ratio(&json!("3/4")).unwrap().p(), 0.75);

        // valid percentage format strings
        assert_eq!(as_random_ratio(&json!("50%")).unwrap().p(), 0.5);
        assert_eq!(as_random_ratio(&json!("100%")).unwrap().p(), 1.0);

        // valid float strings
        assert_eq!(as_random_ratio(&json!("0.7")).unwrap().p(), 0.7);
        assert_eq!(as_random_ratio(&json!("1.0")).unwrap().p(), 1.0);

        // boolean values
        assert_eq!(as_random_ratio(&json!(true)).unwrap().p(), 1.0);
        assert_eq!(as_random_ratio(&json!(false)).unwrap().p(), 0.0);
    }

    #[test]
    fn as_random_ratio_err() {
        // invalid Number values
        assert!(as_random_ratio(&json!(-0.5)).is_err());
        assert!(as_random_ratio(&json!(1.5)).is_err());
        assert!(as_random_ratio(&json!(123)).is_err());

        // invalid fraction format strings
        assert!(as_random_ratio(&json!("a/2")).is_err());
        assert!(as_random_ratio(&json!("1/b")).is_err());
        assert!(as_random_ratio(&json!("3/0")).is_err());
        assert!(as_random_ratio(&json!("1.5/2")).is_err());

        // invalid percentage format strings
        assert!(as_random_ratio(&json!("a%")).is_err());
        assert!(as_random_ratio(&json!("150%")).is_err());

        // invalid float strings
        assert!(as_random_ratio(&json!("abc")).is_err());
        assert!(as_random_ratio(&json!("-0.5")).is_err());
        assert!(as_random_ratio(&json!("2.5")).is_err());

        // unsupported types
        assert!(as_random_ratio(&json!([])).is_err());
        assert!(as_random_ratio(&json!({})).is_err());
        assert!(as_random_ratio(&json!(null)).is_err());
    }
}
