/*
 * SPDX-License-Identifier: Apache-2.0
 * Copyright 2023-2025 ByteDance and/or its affiliates.
 */

use std::collections::HashMap;
use std::hash::Hash;
use std::num::NonZeroU32;
use std::str::FromStr;

use anyhow::{Context, anyhow};
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

pub fn as_bytes(v: &Value, out: &mut [u8]) -> anyhow::Result<()> {
    if let Value::String(s) = v {
        hex::decode_to_slice(s, out).map_err(|e| anyhow!("invalid hex string: {e}"))
    } else {
        Err(anyhow!("json value type for bytes should be 'hex string'"))
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
    use serde_json::{Number, json};

    #[test]
    fn as_u8_ok() {
        // valid string input
        let v = Value::String("123".to_string());
        assert_eq!(as_u8(&v).unwrap(), 123);

        // valid number input
        let v = Value::Number(Number::from(123u32));
        assert_eq!(as_u8(&v).unwrap(), 123);
    }

    #[test]
    fn as_u8_err() {
        // out of range
        let v = Value::Number(Number::from(300u32));
        assert!(as_u8(&v).is_err());

        // negative
        let v = Value::Number(Number::from(-1i32));
        assert!(as_u8(&v).is_err());

        // invalid string
        let v = Value::String("abc".to_string());
        assert!(as_u8(&v).is_err());

        // invalid type
        let v = Value::Bool(true);
        assert!(as_u8(&v).is_err());
    }

    #[test]
    fn as_u16_ok() {
        // valid string input
        let v = Value::String("12345".to_string());
        assert_eq!(as_u16(&v).unwrap(), 12345);

        // valid number input
        let v = Value::Number(Number::from(12345u32));
        assert_eq!(as_u16(&v).unwrap(), 12345);
    }

    #[test]
    fn as_u16_err() {
        // out of range
        let v = Value::Number(Number::from(65536u32));
        assert!(as_u16(&v).is_err());

        // negative
        let v = Value::Number(Number::from(-1i32));
        assert!(as_u16(&v).is_err());

        // invalid string
        let v = Value::String("abc".to_string());
        assert!(as_u16(&v).is_err());

        // invalid type
        let v = Value::Bool(true);
        assert!(as_u16(&v).is_err());
    }

    #[test]
    fn as_i32_ok() {
        // valid string input
        let v = Value::String("-123".to_string());
        assert_eq!(as_i32(&v).unwrap(), -123);

        // valid number input
        let v = Value::Number(Number::from(-123i32));
        assert_eq!(as_i32(&v).unwrap(), -123);
    }

    #[test]
    fn as_i32_err() {
        // out of range
        let v = Value::Number(Number::from(2147483648u64));
        assert!(as_i32(&v).is_err());

        // invalid string
        let v = Value::String("abc".to_string());
        assert!(as_i32(&v).is_err());

        // invalid type
        let v = Value::Bool(true);
        assert!(as_i32(&v).is_err());
    }

    #[test]
    fn as_u32_ok() {
        // valid string input
        let v = Value::String("123456".to_string());
        assert_eq!(as_u32(&v).unwrap(), 123456);

        // valid number input
        let v = Value::Number(Number::from(123456u32));
        assert_eq!(as_u32(&v).unwrap(), 123456);
    }

    #[test]
    fn as_u32_err() {
        // out of range
        let v = Value::Number(Number::from(4294967296u64));
        assert!(as_u32(&v).is_err());

        // negative NonZeroU32
        let v = Value::Number(Number::from(-123i32));
        assert!(as_u32(&v).is_err());

        // invalid string
        let v = Value::String("abc".to_string());
        assert!(as_u32(&v).is_err());

        // invalid type
        let v = Value::Bool(true);
        assert!(as_u32(&v).is_err());
    }

    #[test]
    fn as_nonzero_u32_ok() {
        // valid string input
        let v = Value::String("123".to_string());
        assert_eq!(as_nonzero_u32(&v).unwrap(), NonZeroU32::new(123).unwrap());

        // valid number input
        let v = Value::Number(Number::from(123u32));
        assert_eq!(as_nonzero_u32(&v).unwrap(), NonZeroU32::new(123).unwrap());
    }

    #[test]
    fn as_nonzero_u32_err() {
        // zero value
        let v = Value::String("0".to_string());
        assert!(as_nonzero_u32(&v).is_err());

        let v = Value::Number(Number::from(0u32));
        assert!(as_nonzero_u32(&v).is_err());

        // out of range
        let v = Value::Number(Number::from(4294967296u64));
        assert!(as_nonzero_u32(&v).is_err());

        // negative number
        let v = Value::Number(Number::from(-123i32));
        assert!(as_nonzero_u32(&v).is_err());

        // invalid type
        let v = Value::Bool(true);
        assert!(as_nonzero_u32(&v).is_err());

        let v = Value::Null;
        assert!(as_nonzero_u32(&v).is_err());
    }

    #[test]
    fn as_usize_ok() {
        // valid string input
        let v = Value::String("123".to_string());
        assert_eq!(as_usize(&v).unwrap(), 123);

        // valid number input
        let v = Value::Number(Number::from(123u32));
        assert_eq!(as_usize(&v).unwrap(), 123);
    }

    #[test]
    fn as_usize_err() {
        // negative number
        let v = Value::Number(Number::from(-123i32));
        assert!(as_usize(&v).is_err());

        // invalid string
        let v = Value::String("abc".to_string());
        assert!(as_usize(&v).is_err());

        // invalid type
        let v = Value::Bool(true);
        assert!(as_usize(&v).is_err());

        let v = Value::Null;
        assert!(as_usize(&v).is_err());
    }

    #[test]
    fn as_f64_ok() {
        // valid string input
        let v = Value::String("123.45".to_string());
        assert_eq!(as_f64(&v).unwrap(), 123.45);

        // valid number input
        let v = Value::Number(Number::from_f64(123.45).unwrap());
        assert_eq!(as_f64(&v).unwrap(), 123.45);
    }

    #[test]
    fn as_f64_err() {
        // invalid string
        let v = Value::String("abc".to_string());
        assert!(as_f64(&v).is_err());

        // invalid type
        let v = Value::Bool(true);
        assert!(as_f64(&v).is_err());
    }

    #[test]
    fn as_bool_ok() {
        // string representations
        assert!(as_bool(&Value::String("on".to_string())).unwrap());
        assert!(as_bool(&Value::String("true".to_string())).unwrap());
        assert!(as_bool(&Value::String("1".to_string())).unwrap());
        assert!(!as_bool(&Value::String("off".to_string())).unwrap());
        assert!(!as_bool(&Value::String("false".to_string())).unwrap());
        assert!(!as_bool(&Value::String("0".to_string())).unwrap());

        // boolean values
        assert!(as_bool(&Value::Bool(true)).unwrap());
        assert!(!as_bool(&Value::Bool(false)).unwrap());

        // numeric values
        assert!(as_bool(&Value::Number(Number::from(1))).unwrap());
        assert!(!as_bool(&Value::Number(Number::from(0))).unwrap());
        assert!(as_bool(&Value::Number(Number::from(-1))).unwrap());
    }

    #[test]
    fn as_bool_err() {
        // invalid string
        let v = Value::String("maybe".to_string());
        assert!(as_bool(&v).is_err());

        // unsupported type
        let v = Value::Null;
        assert!(as_bool(&v).is_err());
    }

    #[test]
    fn as_bytes_ok() {
        let mut buffer = [0u8; 4];
        let v = Value::String("deadbeef".to_string());
        as_bytes(&v, &mut buffer).unwrap();
        assert_eq!(buffer, [0xde, 0xad, 0xbe, 0xef]);
    }

    #[test]
    fn as_bytes_err() {
        let mut buffer = [0u8; 4];

        // invalid hex
        let v = Value::String("xxxx".to_string());
        assert!(as_bytes(&v, &mut buffer).is_err());

        // wrong length
        let v = Value::String("dead".to_string());
        assert!(as_bytes(&v, &mut buffer).is_err());

        // invalid type
        let v = Value::Number(Number::from(123));
        assert!(as_bytes(&v, &mut buffer).is_err());
    }

    #[test]
    fn as_ascii_ok() {
        let v = Value::String("hello".to_string());
        assert_eq!(as_ascii(&v).unwrap(), "hello");
    }

    #[test]
    fn as_ascii_err() {
        // non-ASCII string
        let v = Value::String("你好".to_string());
        assert!(as_ascii(&v).is_err());
    }

    #[test]
    fn as_string_ok() {
        // string value
        let v = Value::String("test".to_string());
        assert_eq!(as_string(&v).unwrap(), "test");

        let v = Value::String("123.0".to_string());
        assert_eq!(as_string(&v).unwrap(), "123.0");

        // integer conversion
        let v = Value::Number(Number::from(123u32));
        assert_eq!(as_string(&v).unwrap(), "123");

        // negative integer
        let v = Value::Number(Number::from(-123i32));
        assert_eq!(as_string(&v).unwrap(), "-123");
    }

    #[test]
    fn as_string_err() {
        // float conversion
        let v = Value::Number(Number::from_f64(123.45).unwrap());
        assert!(as_string(&v).is_err());

        // invalid type
        let v = Value::Bool(true);
        assert!(as_string(&v).is_err());
    }

    #[test]
    fn as_list_ok() {
        // array input
        let v = Value::Array(vec![
            Value::Number(Number::from(1)),
            Value::Number(Number::from(2)),
            Value::Number(Number::from(3)),
        ]);
        assert_eq!(as_list(&v, as_u8).unwrap(), vec![1, 2, 3]);

        // single value input
        let v = Value::Number(Number::from(42));
        assert_eq!(as_list(&v, as_u8).unwrap(), vec![42]);
    }

    #[test]
    fn as_list_err() {
        // element conversion failure
        let v = Value::Array(vec![
            Value::Number(Number::from(1)),
            Value::String("invalid".to_string()),
        ]);
        assert!(as_list(&v, as_u8).is_err());
    }

    #[test]
    fn as_hashmap_ok() {
        let v = json!({
            "key1": 1,
            "key2": 2
        });

        let result: HashMap<String, u8> = as_hashmap(&v, |k| Ok(k.to_string()), as_u8).unwrap();

        assert_eq!(result.get("key1"), Some(&1));
        assert_eq!(result.get("key2"), Some(&2));
    }

    #[test]
    fn as_hashmap_err() {
        // non-object input
        let v = Value::Array(vec![]);
        assert!(as_hashmap(&v, |k| Ok(k.to_string()), as_u8).is_err());

        // key conversion failure
        let v = json!({
            "key1": 1,
            "key2": "invalid"
        });
        assert!(as_hashmap(&v, |k| Ok(k.to_string()), as_u8).is_err());
    }
}
