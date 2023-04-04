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

use std::str::FromStr;

use anyhow::{anyhow, Context};
use atoi::FromRadix10;
use rmpv::ValueRef;

use g3_types::collection::WeightedValue;

pub fn as_string(v: &ValueRef) -> anyhow::Result<String> {
    match v {
        ValueRef::String(s) => s
            .as_str()
            .map(|s| s.to_owned())
            .ok_or_else(|| anyhow!("invalid utf-8 string")),
        ValueRef::Binary(b) => {
            let s = std::str::from_utf8(b).map_err(|e| anyhow!("invalid utf-8 string: {e}"))?;
            Ok(s.to_string())
        }
        ValueRef::Integer(i) => Ok(i.to_string()),
        _ => Err(anyhow!(
            "msgpack value type for string should be 'string' / 'binary' / 'integer'"
        )),
    }
}

pub fn as_u32(v: &ValueRef) -> anyhow::Result<u32> {
    match v {
        ValueRef::String(s) => match s.as_str() {
            Some(s) => u32::from_str(s).map_err(|e| anyhow!("invalid u32 string: {e}")),
            None => Err(anyhow!("invalid utf-8 string")),
        },
        ValueRef::Binary(b) => {
            let (v, len) = u32::from_radix_10(b);
            if len != b.len() {
                Err(anyhow!("invalid u32 binary string"))
            } else {
                Ok(v)
            }
        }
        ValueRef::Integer(i) => match i.as_u64() {
            Some(i) => u32::try_from(i).map_err(|e| anyhow!("out of range u32 integer: {e}")),
            None => Err(anyhow!("invalid unsigned integer value")),
        },
        _ => Err(anyhow!(
            "msgpack value type for 'u32' should be 'integer' / 'string' / 'binary'"
        )),
    }
}

pub fn as_f64(v: &ValueRef) -> anyhow::Result<f64> {
    match v {
        ValueRef::Integer(i) => i
            .as_f64()
            .ok_or_else(|| anyhow!("out of range integer value")),
        ValueRef::F64(f) => Ok(*f),
        ValueRef::F32(f) => Ok(*f as f64),
        ValueRef::String(s) => match s.as_str() {
            Some(s) => f64::from_str(s).map_err(|e| anyhow!("invalid f64 string: {e}")),
            None => Err(anyhow!("invalid utf-8 string")),
        },
        _ => Err(anyhow!(
            "msgpack value type for 'f64' should be 'integer' or 'f64' or 'f32' or 'string'"
        )),
    }
}

pub fn as_weighted_name_string(v: &ValueRef) -> anyhow::Result<WeightedValue<String>> {
    match v {
        ValueRef::Map(map) => {
            let mut name = String::new();
            let mut weight = None;

            for (k, v) in map {
                let key = as_string(k).context("all keys should be string")?;
                match crate::key::normalize(key.as_str()).as_str() {
                    "name" => {
                        name =
                            as_string(v).context(format!("invalid string value for key {key}"))?;
                    }
                    "weight" => {
                        let f = crate::value::as_f64(v)
                            .context(format!("invalid f64 value for key {key}"))?;
                        weight = Some(f);
                    }
                    _ => {} // ignore all other keys
                }
            }

            if name.is_empty() {
                Err(anyhow!("no name found"))
            } else if let Some(weight) = weight {
                Ok(WeightedValue::with_weight(name, weight))
            } else {
                Ok(WeightedValue::new(name))
            }
        }
        _ => {
            let s = as_string(v).context("invalid string value")?;
            Ok(WeightedValue::new(s))
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use rmpv::{Integer, Utf8StringRef, ValueRef};

    #[test]
    fn t_string() {
        let v = ValueRef::String(Utf8StringRef::from("123.0"));
        let pv = as_string(&v).unwrap();
        assert_eq!(pv, "123.0");

        let v = ValueRef::Integer(Integer::from(123u32));
        let pv = as_string(&v).unwrap();
        assert_eq!(pv, "123");

        let v = ValueRef::Integer(Integer::from(-123i32));
        let pv = as_string(&v).unwrap();
        assert_eq!(pv, "-123");

        let v = ValueRef::F32(123.0);
        assert!(as_string(&v).is_err());

        let v = ValueRef::Boolean(false);
        assert!(as_string(&v).is_err());
    }

    #[test]
    fn t_u32() {
        let v = ValueRef::String(Utf8StringRef::from("123"));
        let pv = as_u32(&v).unwrap();
        assert_eq!(pv, 123u32);

        let v = ValueRef::Integer(Integer::from(123u64));
        let pv = as_u32(&v).unwrap();
        assert_eq!(pv, 123u32);

        let v = ValueRef::Integer(Integer::from(4_294_967_296u64));
        assert!(as_u32(&v).is_err());

        let v = ValueRef::Integer(Integer::from(-123i32));
        assert!(as_u32(&v).is_err());

        let v = ValueRef::F32(123.0);
        assert!(as_u32(&v).is_err());

        let v = ValueRef::Boolean(false);
        assert!(as_string(&v).is_err());
    }

    #[test]
    fn t_f64() {
        let v = ValueRef::String(Utf8StringRef::from("123"));
        let pv = as_f64(&v).unwrap();
        assert_eq!(pv, 123.0f64);

        let v = ValueRef::String(Utf8StringRef::from("123.0"));
        let pv = as_f64(&v).unwrap();
        assert_eq!(pv, 123.0f64);

        let v = ValueRef::String(Utf8StringRef::from("-123"));
        let pv = as_f64(&v).unwrap();
        assert_eq!(pv, -123.0f64);

        let v = ValueRef::Integer(Integer::from(123u64));
        let pv = as_f64(&v).unwrap();
        assert_eq!(pv, 123.0f64);

        let v = ValueRef::F32(123.0);
        let pv = as_f64(&v).unwrap();
        assert_eq!(pv, 123.0f64);

        let v = ValueRef::F64(123.0);
        let pv = as_f64(&v).unwrap();
        assert_eq!(pv, 123.0f64);

        let v = ValueRef::Boolean(false);
        assert!(as_string(&v).is_err());
    }

    #[test]
    fn t_weighted_name_string() {
        let v = ValueRef::String(Utf8StringRef::from("anc"));
        let pv = as_weighted_name_string(&v).unwrap();
        assert_eq!(pv, WeightedValue::<String>::new("anc".to_string()));

        let v = vec![(
            ValueRef::String(Utf8StringRef::from("name")),
            ValueRef::String(Utf8StringRef::from("anc")),
        )];
        let v = ValueRef::Map(v);
        let pv = as_weighted_name_string(&v).unwrap();
        assert_eq!(pv, WeightedValue::<String>::new("anc".to_string()));

        let v = vec![
            (
                ValueRef::String(Utf8StringRef::from("name")),
                ValueRef::String(Utf8StringRef::from("anc")),
            ),
            (
                ValueRef::String(Utf8StringRef::from("weight")),
                ValueRef::Integer(Integer::from(1)),
            ),
        ];
        let v = ValueRef::Map(v);
        let pv = as_weighted_name_string(&v).unwrap();
        assert_eq!(pv, WeightedValue::<String>::new("anc".to_string()));
    }
}
