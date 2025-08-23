/*
 * SPDX-License-Identifier: Apache-2.0
 * Copyright 2023-2025 ByteDance and/or its affiliates.
 */

use std::collections::HashMap;
use std::hash::Hash;
use std::num::{NonZeroI32, NonZeroIsize, NonZeroU32, NonZeroUsize};
use std::str::FromStr;

use anyhow::{Context, anyhow};
use ascii::AsciiString;
use yaml_rust::Yaml;

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

pub fn as_nonzero_usize(v: &Yaml) -> anyhow::Result<NonZeroUsize> {
    match v {
        Yaml::String(s) => Ok(NonZeroUsize::from_str(s)?),
        Yaml::Integer(i) => {
            let u = usize::try_from(*i)?;
            Ok(NonZeroUsize::try_from(u)?)
        }
        _ => Err(anyhow!(
            "yaml value type for 'nonzero usize' should be 'string' or 'integer'"
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

#[cfg(test)]
mod tests {
    use super::*;
    use yaml_rust::YamlLoader;

    #[test]
    fn as_u8_ok() {
        // valid string input
        let v = yaml_str!("100");
        assert_eq!(as_u8(&v).unwrap(), 100);

        // valid integer input
        let v = Yaml::Integer(200);
        assert_eq!(as_u8(&v).unwrap(), 200);

        // boundary value (max u8)
        let v = Yaml::Integer(255);
        assert_eq!(as_u8(&v).unwrap(), 255);
    }

    #[test]
    fn as_u8_err() {
        // overflow
        let v = Yaml::Integer(256);
        assert!(as_u8(&v).is_err());

        // negative number
        let v = Yaml::Integer(-1);
        assert!(as_u8(&v).is_err());

        // invalid type
        let v = Yaml::Boolean(true);
        assert!(as_u8(&v).is_err());

        // parse error
        let v = yaml_str!("abc");
        assert!(as_u8(&v).is_err());
    }

    #[test]
    fn as_u16_ok() {
        // valid string input
        let v = yaml_str!("50000");
        assert_eq!(as_u16(&v).unwrap(), 50000);

        // valid integer input
        let v = Yaml::Integer(60000);
        assert_eq!(as_u16(&v).unwrap(), 60000);

        // boundary value (max u16)
        let v = Yaml::Integer(65535);
        assert_eq!(as_u16(&v).unwrap(), 65535);
    }

    #[test]
    fn as_u16_err() {
        // overflow
        let v = Yaml::Integer(65536);
        assert!(as_u16(&v).is_err());

        // negative number
        let v = Yaml::Integer(-1);
        assert!(as_u16(&v).is_err());

        // invalid type
        let v = Yaml::Boolean(false);
        assert!(as_u16(&v).is_err());

        // parse error
        let v = yaml_str!("def");
        assert!(as_u16(&v).is_err());
    }

    #[test]
    fn as_u32_ok() {
        // valid string input
        let v = yaml_str!("4000000000");
        assert_eq!(as_u32(&v).unwrap(), 4000000000);

        // valid integer input
        let v = Yaml::Integer(2000000000);
        assert_eq!(as_u32(&v).unwrap(), 2000000000);

        // boundary value (max u32)
        let v = Yaml::Integer(4294967295);
        assert_eq!(as_u32(&v).unwrap(), 4294967295);
    }

    #[test]
    fn as_u32_err() {
        // overflow
        let v = Yaml::Integer(4294967296);
        assert!(as_u32(&v).is_err());

        // negative number
        let v = Yaml::Integer(-1);
        assert!(as_u32(&v).is_err());

        // invalid type
        let v = Yaml::Null;
        assert!(as_u32(&v).is_err());

        // parse error
        let v = yaml_str!("ghi");
        assert!(as_u32(&v).is_err());
    }

    #[test]
    fn as_nonzero_u32_ok() {
        // valid string input
        let v = yaml_str!("1");
        assert_eq!(as_nonzero_u32(&v).unwrap(), NonZeroU32::new(1).unwrap());

        // valid integer input
        let v = Yaml::Integer(2);
        assert_eq!(as_nonzero_u32(&v).unwrap(), NonZeroU32::new(2).unwrap());

        // boundary value (max u32)
        let v = Yaml::Integer(4294967295);
        assert_eq!(
            as_nonzero_u32(&v).unwrap(),
            NonZeroU32::new(4294967295).unwrap()
        );
    }

    #[test]
    fn as_nonzero_u32_err() {
        // overflow
        let v = Yaml::Integer(4294967296);
        assert!(as_nonzero_u32(&v).is_err());

        // zero value
        let v = yaml_str!("0");
        assert!(as_nonzero_u32(&v).is_err());

        // negative number
        let v = Yaml::Integer(-1);
        assert!(as_nonzero_u32(&v).is_err());

        // invalid type
        let v = Yaml::Array(vec![]);
        assert!(as_nonzero_u32(&v).is_err());

        // parse error
        let v = yaml_str!("jkl");
        assert!(as_nonzero_u32(&v).is_err());
    }

    #[test]
    fn as_u64_ok() {
        // valid string input
        let v = yaml_str!("18446744073709551615");
        assert_eq!(as_u64(&v).unwrap(), 18446744073709551615);

        // valid integer input
        let v = Yaml::Integer(8446744073709551615);
        assert_eq!(as_u64(&v).unwrap(), 8446744073709551615);
    }

    #[test]
    fn as_u64_err() {
        // overflow
        let v = yaml_str!("18446744073709551616");
        assert!(as_u64(&v).is_err());

        // negative number
        let v = Yaml::Integer(-1);
        assert!(as_u64(&v).is_err());

        // invalid type
        let v = Yaml::Boolean(false);
        assert!(as_u64(&v).is_err());

        // parse error
        let v = yaml_str!("mno");
        assert!(as_u64(&v).is_err());
    }

    #[test]
    fn as_i32_ok() {
        // valid string input
        let v = yaml_str!("-2147483648");
        assert_eq!(as_i32(&v).unwrap(), -2147483648);

        let v = yaml_str!("0");
        assert_eq!(as_i32(&v).unwrap(), 0);

        // valid integer input
        let v = Yaml::Integer(2147483647);
        assert_eq!(as_i32(&v).unwrap(), 2147483647);
    }

    #[test]
    fn as_i32_err() {
        // overflow
        let v = Yaml::Integer(2147483648);
        assert!(as_i32(&v).is_err());

        // underflow
        let v = Yaml::Integer(-2147483649);
        assert!(as_i32(&v).is_err());

        // invalid type
        let v = Yaml::Boolean(true);
        assert!(as_i32(&v).is_err());

        // parse error
        let v = yaml_str!("pqr");
        assert!(as_i32(&v).is_err());
    }

    #[test]
    fn as_nonzero_i32_ok() {
        // valid positive value
        let v = yaml_str!(1);
        assert_eq!(as_nonzero_i32(&v).unwrap(), NonZeroI32::new(1).unwrap());

        let v = Yaml::Integer(2147483647);
        assert_eq!(
            as_nonzero_i32(&v).unwrap(),
            NonZeroI32::new(2147483647).unwrap()
        );

        // valid negative value
        let v = yaml_str!(-1);
        assert_eq!(as_nonzero_i32(&v).unwrap(), NonZeroI32::new(-1).unwrap());

        let v = Yaml::Integer(-2147483648);
        assert_eq!(
            as_nonzero_i32(&v).unwrap(),
            NonZeroI32::new(-2147483648).unwrap()
        );
    }

    #[test]
    fn as_nonzero_i32_err() {
        // zero value
        let v = yaml_str!("0");
        assert!(as_nonzero_i32(&v).is_err());

        // overflow
        let v = Yaml::Integer(2147483648);
        assert!(as_nonzero_i32(&v).is_err());

        // underflow
        let v = Yaml::Integer(-2147483649);
        assert!(as_nonzero_i32(&v).is_err());

        // invalid type
        let v = Yaml::Null;
        assert!(as_nonzero_i32(&v).is_err());

        // parse error
        let v = yaml_str!("stu");
        assert!(as_nonzero_i32(&v).is_err());
    }

    #[test]
    fn as_i64_ok() {
        // valid string input
        let v = yaml_str!("-9223372036854775808");
        assert_eq!(as_i64(&v).unwrap(), -9223372036854775808);

        let v = yaml_str!("0");
        assert_eq!(as_i64(&v).unwrap(), 0);

        // valid integer input
        let v = Yaml::Integer(9223372036854775807);
        assert_eq!(as_i64(&v).unwrap(), 9223372036854775807);
    }

    #[test]
    fn as_i64_err() {
        // overflow
        let v = yaml_str!("9223372036854775808");
        assert!(as_i64(&v).is_err());

        // underflow
        let v = yaml_str!("-9223372036854775809");
        assert!(as_i64(&v).is_err());

        // invalid type
        let v = Yaml::Real("1.234e10".into());
        assert!(as_i64(&v).is_err());

        // parse error
        let v = yaml_str!("xyz");
        assert!(as_i64(&v).is_err());
    }

    #[test]
    fn as_f64_ok() {
        // valid string input
        let v = yaml_str!("3.141592653589793");
        assert_eq!(as_f64(&v).unwrap(), std::f64::consts::PI);

        // valid integer input
        let v = Yaml::Integer(42);
        assert_eq!(as_f64(&v).unwrap(), 42.0);

        // valid real input
        let v = Yaml::Real("1.234e10".into());
        assert_eq!(as_f64(&v).unwrap(), 1.234e10);
    }

    #[test]
    fn as_f64_err() {
        // invalid string
        let v = yaml_str!("not a number");
        assert!(as_f64(&v).is_err());

        // invalid type
        let v = Yaml::Boolean(true);
        assert!(as_f64(&v).is_err());
    }

    #[test]
    fn as_bool_ok() {
        // truthy strings
        assert!(as_bool(&yaml_str!("on")).unwrap());
        assert!(as_bool(&yaml_str!("true")).unwrap());
        assert!(as_bool(&yaml_str!("yes")).unwrap());
        assert!(as_bool(&yaml_str!("1")).unwrap());

        // falsy strings
        assert!(!as_bool(&yaml_str!("off")).unwrap());
        assert!(!as_bool(&yaml_str!("false")).unwrap());
        assert!(!as_bool(&yaml_str!("no")).unwrap());
        assert!(!as_bool(&yaml_str!("0")).unwrap());

        // boolean values
        assert!(as_bool(&Yaml::Boolean(true)).unwrap());
        assert!(!as_bool(&Yaml::Boolean(false)).unwrap());

        // integer values
        assert!(as_bool(&Yaml::Integer(1)).unwrap());
        assert!(!as_bool(&Yaml::Integer(0)).unwrap());
    }

    #[test]
    fn as_bool_err() {
        // invalid string
        let v = yaml_str!("maybe");
        assert!(as_bool(&v).is_err());

        // invalid type
        let v = Yaml::Real("123.45".into());
        assert!(as_bool(&v).is_err());
    }

    #[test]
    fn as_nonzero_isize_ok() {
        // positive value
        let v = Yaml::Integer(1);
        assert_eq!(as_nonzero_isize(&v).unwrap(), NonZeroIsize::new(1).unwrap());

        let v = yaml_str!("2");
        assert_eq!(as_nonzero_isize(&v).unwrap(), NonZeroIsize::new(2).unwrap());

        // negative value
        let v = Yaml::Integer(-1);
        assert_eq!(
            as_nonzero_isize(&v).unwrap(),
            NonZeroIsize::new(-1).unwrap()
        );

        let v = yaml_str!("-2");
        assert_eq!(
            as_nonzero_isize(&v).unwrap(),
            NonZeroIsize::new(-2).unwrap()
        );
    }

    #[test]
    fn as_nonzero_isize_err() {
        // zero value
        let v = Yaml::Integer(0);
        assert!(as_nonzero_isize(&v).is_err());

        let v = yaml_str!("0");
        assert!(as_nonzero_isize(&v).is_err());

        // invalid type
        let v = Yaml::Null;
        assert!(as_nonzero_isize(&v).is_err());
    }

    #[test]
    fn as_usize_ok() {
        // valid string input
        let v = yaml_str!("100");
        assert_eq!(as_usize(&v).unwrap(), 100);

        // valid integer input
        let v = Yaml::Integer(200);
        assert_eq!(as_usize(&v).unwrap(), 200);
    }

    #[test]
    fn as_usize_err() {
        // negative number
        let v = Yaml::Integer(-1);
        assert!(as_usize(&v).is_err());

        // invalid type
        let v = Yaml::Array(vec![]);
        assert!(as_usize(&v).is_err());
    }

    #[test]
    fn as_nonzero_usize_ok() {
        // valid string input
        let v = yaml_str!("1");
        assert_eq!(as_nonzero_usize(&v).unwrap(), NonZeroUsize::new(1).unwrap());

        // valid integer input
        let v = Yaml::Integer(2);
        assert_eq!(as_nonzero_usize(&v).unwrap(), NonZeroUsize::new(2).unwrap());
    }

    #[test]
    fn as_nonzero_usize_err() {
        // zero value
        let v = yaml_str!("0");
        assert!(as_nonzero_usize(&v).is_err());

        // negative number
        let v = Yaml::Integer(-1);
        assert!(as_nonzero_usize(&v).is_err());

        // invalid type
        let v = Yaml::Null;
        assert!(as_nonzero_usize(&v).is_err());
    }

    #[test]
    fn as_ascii_ok() {
        // valid ASCII string
        let v = yaml_str!("hello");
        assert_eq!(as_ascii(&v).unwrap().as_str(), "hello");
    }

    #[test]
    fn as_ascii_err() {
        // non-ASCII string
        let v = yaml_str!("héllo");
        assert!(as_ascii(&v).is_err());

        let v = yaml_str!("你好");
        assert!(as_ascii(&v).is_err());

        // invalid type
        let v = Yaml::Array(vec![]);
        assert!(as_ascii(&v).is_err());
    }

    #[test]
    fn as_string_ok() {
        // Valid string
        let v = yaml_str!("123.0");
        assert_eq!(as_string(&v).unwrap(), "123.0");

        // Valid integer
        let v = Yaml::Integer(123);
        assert_eq!(as_string(&v).unwrap(), "123");

        // Valid negative integer
        let v = Yaml::Integer(-123);
        assert_eq!(as_string(&v).unwrap(), "-123");

        // Valid real number
        let v = Yaml::Real("123.0".into());
        assert_eq!(as_string(&v).unwrap(), "123.0");
    }

    #[test]
    fn as_string_err() {
        // Invalid type (boolean)
        let v = Yaml::Boolean(true);
        assert!(as_string(&v).is_err());

        // Invalid type (null)
        let v = Yaml::Null;
        assert!(as_string(&v).is_err());
    }

    #[test]
    fn as_list_ok() {
        // array input
        let v = Yaml::Array(vec![Yaml::Integer(1), Yaml::Integer(2), Yaml::Integer(3)]);
        assert_eq!(as_list(&v, as_i32).unwrap(), vec![1, 2, 3]);

        let v = yaml_doc!("[-1, -2, -3]");
        assert_eq!(as_list(&v, as_i32).unwrap(), vec![-1, -2, -3]);

        // single value
        let v = yaml_doc!("42");
        assert_eq!(as_list(&v, as_i32).unwrap(), vec![42]);
    }

    #[test]
    fn as_list_err() {
        // conversion error in element
        let v = Yaml::Array(vec![
            Yaml::Integer(1),
            Yaml::Boolean(true),
            Yaml::Integer(3),
        ]);
        assert!(as_list::<i32, _>(&v, as_i32).is_err());

        let v = yaml_doc!("[1, 2, 3, x]");
        assert!(as_list::<i32, _>(&v, as_i32).is_err());

        // invalid single value
        let v = yaml_doc!("notanumber");
        assert!(as_list::<i32, _>(&v, as_i32).is_err());

        // invalid type
        let v = Yaml::Null;
        assert!(as_list::<i32, _>(&v, as_i32).is_err());
    }

    #[test]
    fn as_hashmap_ok() {
        // valid map
        let v = yaml_doc!(
            "
            key1: 1
            key2: 2
            "
        );
        let result = as_hashmap(&v, as_string, as_i32).unwrap();
        assert_eq!(result.get("key1").unwrap(), &1);
        assert_eq!(result.get("key2").unwrap(), &2);

        let v = yaml_doc!("key: value");
        assert_eq!(
            as_hashmap(&v, as_string, as_string)
                .unwrap()
                .get("key")
                .unwrap(),
            "value"
        );

        // empty map
        let v = yaml_doc!("{}");
        assert_eq!(as_hashmap(&v, as_string, as_i32).unwrap().len(), 0);
    }

    #[test]
    fn as_hashmap_err() {
        // non-key
        let v = yaml_doc!(": value");
        assert!(as_hashmap(&v, as_string, as_string).is_err());

        // invalid value
        let v = yaml_doc!("key: not_a_number");
        assert!(as_hashmap(&v, as_string, as_i32).is_err());

        // invalid type
        let v = Yaml::Null;
        assert!(as_hashmap(&v, as_string, as_i32).is_err());
    }
}
