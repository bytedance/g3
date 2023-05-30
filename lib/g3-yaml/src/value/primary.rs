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
use std::convert::TryFrom;
use std::hash::Hash;
use std::num::{NonZeroI32, NonZeroIsize, NonZeroU32};
use std::str::FromStr;

use anyhow::{anyhow, Context};
use ascii::AsciiString;
use yaml_rust::Yaml;

use g3_types::collection::WeightedValue;

pub fn as_u8(v: &Yaml) -> anyhow::Result<u8> {
    match v {
        Yaml::String(s) => Ok(u8::from_str(s)?),
        Yaml::Integer(i) => Ok(u8::try_from(*i)?),
        _ => Err(anyhow!(
            "yaml value type for 'u8' should be 'string' or 'integer'"
        )),
    }
}

pub fn as_u16(v: &Yaml) -> anyhow::Result<u16> {
    match v {
        Yaml::String(s) => Ok(u16::from_str(s)?),
        Yaml::Integer(i) => Ok(u16::try_from(*i)?),
        _ => Err(anyhow!(
            "yaml value type for 'u16' should be 'string' or 'integer'"
        )),
    }
}

pub fn as_u32(v: &Yaml) -> anyhow::Result<u32> {
    match v {
        Yaml::String(s) => Ok(u32::from_str(s)?),
        Yaml::Integer(i) => Ok(u32::try_from(*i)?),
        _ => Err(anyhow!(
            "yaml value type for 'u32' should be 'string' or 'integer'"
        )),
    }
}

pub fn as_nonzero_u32(v: &Yaml) -> anyhow::Result<NonZeroU32> {
    match v {
        Yaml::String(s) => Ok(NonZeroU32::from_str(s)?),
        Yaml::Integer(i) => {
            let u = u32::try_from(*i)?;
            Ok(NonZeroU32::try_from(u)?)
        }
        _ => Err(anyhow!(
            "yaml value type for 'nonzero u32' should be 'string' or 'integer'"
        )),
    }
}

pub fn as_u64(v: &Yaml) -> anyhow::Result<u64> {
    match v {
        Yaml::String(s) => Ok(u64::from_str(s)?),
        Yaml::Integer(i) => Ok(u64::try_from(*i)?),
        _ => Err(anyhow!(
            "yaml value type for 'u64' should be 'string' or 'integer'"
        )),
    }
}

pub fn as_i32(v: &Yaml) -> anyhow::Result<i32> {
    match v {
        Yaml::String(s) => Ok(i32::from_str(s)?),
        Yaml::Integer(i) => Ok(i32::try_from(*i)?),
        _ => Err(anyhow!(
            "yaml value type for 'i32' should be 'string' or 'integer'"
        )),
    }
}

pub fn as_nonzero_i32(v: &Yaml) -> anyhow::Result<NonZeroI32> {
    match v {
        Yaml::String(s) => Ok(NonZeroI32::from_str(s)?),
        Yaml::Integer(i) => {
            let u = i32::try_from(*i)?;
            Ok(NonZeroI32::try_from(u)?)
        }
        _ => Err(anyhow!(
            "yaml value type for 'nonzero i32' should be 'string' or 'integer'"
        )),
    }
}

pub fn as_i64(v: &Yaml) -> anyhow::Result<i64> {
    match v {
        Yaml::String(s) => Ok(i64::from_str(s)?),
        Yaml::Integer(i) => Ok(*i),
        _ => Err(anyhow!(
            "yaml value type for 'i64' should be 'string' or 'integer'"
        )),
    }
}

pub fn as_f64(v: &Yaml) -> anyhow::Result<f64> {
    match v {
        Yaml::String(s) => Ok(f64::from_str(s)?),
        Yaml::Integer(i) => Ok(*i as f64),
        Yaml::Real(s) => Ok(f64::from_str(s)?),
        _ => Err(anyhow!(
            "yaml value type for 'f64' should be 'string', 'integer' or 'real'"
        )),
    }
}

pub fn as_bool(v: &Yaml) -> anyhow::Result<bool> {
    match v {
        Yaml::String(s) => match s.to_lowercase().as_str() {
            "on" | "true" | "yes" | "1" => Ok(true),
            "off" | "false" | "no" | "0" => Ok(false),
            _ => Err(anyhow!("invalid yaml string value for 'bool': {s}")),
        },
        Yaml::Boolean(value) => Ok(*value),
        Yaml::Integer(i) => Ok(*i != 0),
        _ => Err(anyhow!(
            "yaml value type for 'bool' should be 'boolean' / 'string' / 'integer'"
        )),
    }
}

pub fn as_nonzero_isize(v: &Yaml) -> anyhow::Result<NonZeroIsize> {
    match v {
        Yaml::String(s) => Ok(NonZeroIsize::from_str(s)?),
        Yaml::Integer(i) => {
            let u = isize::try_from(*i)?;
            Ok(NonZeroIsize::try_from(u)?)
        }
        _ => Err(anyhow!(
            "yaml value type for 'nonzero isize' should be 'string' or 'integer'"
        )),
    }
}

pub fn as_usize(v: &Yaml) -> anyhow::Result<usize> {
    match v {
        Yaml::String(s) => Ok(usize::from_str(s)?),
        Yaml::Integer(i) => Ok(usize::try_from(*i)?),
        _ => Err(anyhow!(
            "yaml value type for 'usize' should be 'string' or 'integer'"
        )),
    }
}

pub fn as_ascii(v: &Yaml) -> anyhow::Result<AsciiString> {
    let s = as_string(v).context("the base type for AsciiString should be String")?;
    AsciiString::from_str(&s).map_err(|e| anyhow!("invalid ascii string: {e}"))
}

pub fn as_string(v: &Yaml) -> anyhow::Result<String> {
    match v {
        Yaml::String(s) => Ok(s.to_string()),
        Yaml::Integer(i) => Ok(i.to_string()),
        Yaml::Real(s) => Ok(s.to_string()),
        _ => Err(anyhow!(
            "yaml value type for string should be 'string' / 'integer' / 'real'"
        )),
    }
}

pub fn as_list<T, F>(v: &Yaml, convert: F) -> anyhow::Result<Vec<T>>
where
    F: Fn(&Yaml) -> anyhow::Result<T>,
{
    let mut vec = Vec::new();
    match v {
        Yaml::Array(seq) => {
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
    v: &Yaml,
    convert_key: KF,
    convert_value: VF,
) -> anyhow::Result<HashMap<K, V>>
where
    K: Hash + Eq,
    KF: Fn(&Yaml) -> anyhow::Result<K>,
    VF: Fn(&Yaml) -> anyhow::Result<V>,
{
    if let Yaml::Hash(map) = v {
        let mut table = HashMap::new();
        for (k, v) in map.iter() {
            let key = convert_key(k).context(format!("failed to parse key {k:?}"))?;
            let value = convert_value(v).context(format!("failed to parse value for key {k:?}"))?;
            table.insert(key, value);
        }
        Ok(table)
    } else {
        Err(anyhow!("the yaml value should be a 'map'"))
    }
}

pub fn as_weighted_name_string(value: &Yaml) -> anyhow::Result<WeightedValue<String>> {
    const KEY_NAME: &str = "name";
    const KEY_WEIGHT: &str = "weight";

    match value {
        Yaml::String(s) => Ok(WeightedValue::<String>::new(s.to_string())),
        Yaml::Hash(map) => {
            let v = crate::hash::get_required(map, KEY_NAME)?;
            let name = as_string(v).context(format!("invalid string value for key {KEY_NAME}"))?;

            if let Ok(v) = crate::hash::get_required(map, KEY_WEIGHT) {
                let weight =
                    as_f64(v).context(format!("invalid f64 value for key {KEY_WEIGHT}"))?;
                Ok(WeightedValue::<String>::with_weight(name, weight))
            } else {
                Ok(WeightedValue::new(name))
            }
        }
        _ => {
            let s = as_string(value).context("invalid string value")?;
            Ok(WeightedValue::new(s))
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn t_string() {
        let v = Yaml::String("123.0".to_string());
        let pv = as_string(&v).unwrap();
        assert_eq!(pv, "123.0");

        let v = Yaml::Integer(123);
        let pv = as_string(&v).unwrap();
        assert_eq!(pv, "123");

        let v = Yaml::Integer(-123);
        let pv = as_string(&v).unwrap();
        assert_eq!(pv, "-123");

        let v = Yaml::Real("123.0".to_string());
        let pv = as_string(&v).unwrap();
        assert_eq!(pv, "123.0");
    }
}
