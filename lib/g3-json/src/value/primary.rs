/*
 * Copyright 2023 ByteDance and/or its affiliates.
 *
 * Licensed under the Apache License, Version 2.0 (the "License");
 * you may not use this file except in compliance with the License.
 * You may obtain a copy of the License at
 *
 *     http://www.apache.org/licenses/LICENSE-2.0
 *
 * Unless required by applicable law or agreed to in writing, software
 * distributed under the License is distributed on an "AS IS" BASIS,
 * WITHOUT WARRANTIES OR CONDITIONS OF ANY KIND, either express or implied.
 * See the License for the specific language governing permissions and
 * limitations under the License.
 */

use std::collections::HashMap;
use std::hash::Hash;
use std::num::NonZeroU32;
use std::str::FromStr;

use anyhow::{anyhow, Context};
use ascii::AsciiString;
use serde_json::Value;

pub fn as_u8(v: &Value) -> anyhow::Result<u8> {
    match v {
        Value::String(s) => Ok(u8::from_str(s)?),
        Value::Number(n) => {
            if let Some(n) = n.as_u64() {
                Ok(u8::try_from(n)?)
            } else {
                Err(anyhow!("out of range json value for u8"))
            }
        }
        _ => Err(anyhow!(
            "json value type for 'u8' should be 'string' or 'positive integer'"
        )),
    }
}

pub fn as_u16(v: &Value) -> anyhow::Result<u16> {
    match v {
        Value::String(s) => Ok(u16::from_str(s)?),
        Value::Number(n) => {
            if let Some(n) = n.as_u64() {
                Ok(u16::try_from(n)?)
            } else {
                Err(anyhow!("out of range json value for u16"))
            }
        }
        _ => Err(anyhow!(
            "json value type for 'u16' should be 'string' or 'positive integer'"
        )),
    }
}

pub fn as_i32(v: &Value) -> anyhow::Result<i32> {
    match v {
        Value::String(s) => Ok(i32::from_str(s)?),
        Value::Number(n) => {
            if let Some(i) = n.as_i64() {
                Ok(i32::try_from(i)?)
            } else {
                Err(anyhow!("out of range json value for i32"))
            }
        }
        _ => Err(anyhow!(
            "json value type for 'i32' should be 'string' or 'integer'"
        )),
    }
}

pub fn as_u32(v: &Value) -> anyhow::Result<u32> {
    match v {
        Value::String(s) => Ok(u32::from_str(s)?),
        Value::Number(n) => {
            if let Some(n) = n.as_u64() {
                Ok(u32::try_from(n)?)
            } else {
                Err(anyhow!("out of range json value for u32"))
            }
        }
        _ => Err(anyhow!(
            "json value type for 'u32' should be 'string' or 'positive integer'"
        )),
    }
}

pub fn as_nonzero_u32(v: &Value) -> anyhow::Result<NonZeroU32> {
    match v {
        Value::String(s) => Ok(NonZeroU32::from_str(s)?),
        Value::Number(n) => {
            if let Some(u) = n.as_u64() {
                let u = u32::try_from(u)?;
                Ok(NonZeroU32::try_from(u)?)
            } else {
                Err(anyhow!("out of range json value for nonzero u32"))
            }
        }
        _ => Err(anyhow!(
            "json value type for 'nonzero u32' should be 'string' or 'positive integer'"
        )),
    }
}

pub fn as_usize(v: &Value) -> anyhow::Result<usize> {
    match v {
        Value::String(s) => Ok(usize::from_str(s)?),
        Value::Number(n) => {
            if let Some(n) = n.as_u64() {
                Ok(usize::try_from(n)?)
            } else {
                Err(anyhow!("out of range json value for usize"))
            }
        }
        _ => Err(anyhow!(
            "json value type for 'usize' should be 'string' or 'positive integer'"
        )),
    }
}

pub fn as_f64(v: &Value) -> anyhow::Result<f64> {
    match v {
        Value::String(s) => Ok(f64::from_str(s)?),
        Value::Number(n) => {
            if let Some(f) = n.as_f64() {
                Ok(f)
            } else {
                Err(anyhow!("out of range json value for f64"))
            }
        }
        _ => Err(anyhow!(
            "json value type for 'f64' should be 'string' or 'number'"
        )),
    }
}

pub fn as_bool(v: &Value) -> anyhow::Result<bool> {
    match v {
        Value::String(s) => match s.to_lowercase().as_str() {
            "on" | "true" | "1" => Ok(true),
            "off" | "false" | "0" => Ok(false),
            _ => Err(anyhow!("invalid yaml string value for 'bool': {s}")),
        },
        Value::Bool(value) => Ok(*value),
        Value::Number(i) => {
            if let Some(n) = i.as_u64() {
                Ok(n != 0)
            } else if let Some(n) = i.as_i64() {
                Ok(n != 0)
            } else {
                Err(anyhow!("json real value can not be used as boolean value"))
            }
        }
        _ => Err(anyhow!(
            "json value type for 'bool' should be 'boolean' / 'string' / 'number'"
        )),
    }
}

pub fn as_ascii(v: &Value) -> anyhow::Result<AsciiString> {
    let s = as_string(v).context("the base type for AsciiString should be String")?;
    AsciiString::from_str(&s).map_err(|e| anyhow!("invalid ascii string: {e}"))
}

pub fn as_string(v: &Value) -> anyhow::Result<String> {
    match v {
        Value::String(s) => Ok(s.to_string()),
        Value::Number(n) => {
            if let Some(i) = n.as_i64() {
                Ok(i.to_string())
            } else if let Some(u) = n.as_u64() {
                Ok(u.to_string())
            } else {
                Err(anyhow!("float/real value can not be used as string"))
            }
        }
        _ => Err(anyhow!(
            "json value type for string should be 'string' / 'integer'"
        )),
    }
}

pub fn as_list<T, F>(v: &Value, convert: F) -> anyhow::Result<Vec<T>>
where
    F: Fn(&Value) -> anyhow::Result<T>,
{
    let mut vec = Vec::new();
    match v {
        Value::Array(seq) => {
            for (i, v) in seq.iter().enumerate() {
                let node = convert(v).context(format!("invalid value for list element #{i}"))?;
                vec.push(node);
            }
        }
        _ => {
            let node = convert(v).context("invalid single value for the list")?;
            vec.push(node);
        }
    }
    Ok(vec)
}

pub fn as_hashmap<K, V, KF, VF>(
    v: &Value,
    convert_key: KF,
    convert_value: VF,
) -> anyhow::Result<HashMap<K, V>>
where
    K: Hash + Eq,
    KF: Fn(&str) -> anyhow::Result<K>,
    VF: Fn(&Value) -> anyhow::Result<V>,
{
    if let Value::Object(map) = v {
        let mut table = HashMap::new();
        for (k, v) in map.iter() {
            let key = convert_key(k).context(format!("failed to parse key {k:?}"))?;
            let value = convert_value(v).context(format!("failed to parse value for key {k:?}"))?;
            table.insert(key, value);
        }
        Ok(table)
    } else {
        Err(anyhow!("the json value should be a 'map'"))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::Number;

    #[test]
    fn t_string() {
        let v = Value::String("123.0".to_string());
        let pv = as_string(&v).unwrap();
        assert_eq!(pv, "123.0");

        let v = Value::Number(Number::from(123u32));
        let pv = as_string(&v).unwrap();
        assert_eq!(pv, "123");

        let v = Value::Number(Number::from(-123i32));
        let pv = as_string(&v).unwrap();
        assert_eq!(pv, "-123");

        let v = Value::Number(Number::from_f64(123.0).unwrap());
        assert!(as_string(&v).is_err());
    }
}
