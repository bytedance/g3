/*
 * SPDX-License-Identifier: Apache-2.0
 * Copyright 2023-2025 ByteDance and/or its affiliates.
 */

use anyhow::anyhow;
use humanize_rs::bytes::Bytes;
use yaml_rust::Yaml;

pub fn as_usize(v: &Yaml) -> anyhow::Result<usize> {
    match v {
        Yaml::String(value) => {
            let v = value.parse::<Bytes>()?;
            Ok(v.size())
        }
        Yaml::Integer(value) => Ok(usize::try_from(*value)?),
        _ => Err(anyhow!(
            "yaml value type for humanize usize should be 'string' or 'integer'"
        )),
    }
}

pub fn as_u64(v: &Yaml) -> anyhow::Result<u64> {
    match v {
        Yaml::String(value) => {
            let v = value.parse::<Bytes<u64>>()?;
            Ok(v.size())
        }
        Yaml::Integer(value) => Ok(u64::try_from(*value)?),
        _ => Err(anyhow!(
            "yaml value type for humanize u64 should be 'string' or 'integer'"
        )),
    }
}

pub fn as_u32(v: &Yaml) -> anyhow::Result<u32> {
    match v {
        Yaml::String(value) => {
            let v = value.parse::<Bytes<u32>>()?;
            Ok(v.size())
        }
        Yaml::Integer(value) => Ok(u32::try_from(*value)?),
        _ => Err(anyhow!(
            "yaml value type for humanize u32 should be 'string' or 'integer'"
        )),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn as_usize_ok() {
        let v = yaml_str!("1000");
        assert_eq!(as_usize(&v).unwrap(), 1000);

        let v = yaml_str!("1K");
        assert_eq!(as_usize(&v).unwrap(), 1000);

        let v = yaml_str!("1KB");
        assert_eq!(as_usize(&v).unwrap(), 1000);

        let v = yaml_str!("1KiB");
        assert_eq!(as_usize(&v).unwrap(), 1024);

        let v = Yaml::Integer(1024);
        assert_eq!(as_usize(&v).unwrap(), 1024);
    }

    #[test]
    fn as_usize_err() {
        let v = Yaml::Integer(-1024);
        assert!(as_usize(&v).is_err());

        let v = Yaml::Real("1.01".to_string());
        assert!(as_usize(&v).is_err());

        let v = Yaml::Array(vec![Yaml::Integer(1)]);
        assert!(as_usize(&v).is_err());
    }

    #[test]
    fn as_u64_ok() {
        let v = yaml_str!("2000");
        assert_eq!(as_u64(&v).unwrap(), 2000);

        let v = Yaml::Integer(2048);
        assert_eq!(as_u64(&v).unwrap(), 2048);
    }

    #[test]
    fn as_u64_err() {
        let v = Yaml::Integer(-2048);
        assert!(as_u64(&v).is_err());

        let v = Yaml::Real("2.02".to_string());
        assert!(as_u64(&v).is_err());

        let v = Yaml::Boolean(true);
        assert!(as_u64(&v).is_err());
    }

    #[test]
    fn as_u32_ok() {
        let v = yaml_str!("4000");
        assert_eq!(as_u32(&v).unwrap(), 4000);

        let v = Yaml::Integer(4096);
        assert_eq!(as_u32(&v).unwrap(), 4096);
    }

    #[test]
    fn as_u32_err() {
        let v = Yaml::Integer(-4096);
        assert!(as_u32(&v).is_err());

        let v = Yaml::Real("4.04".to_string());
        assert!(as_u32(&v).is_err());

        let v = Yaml::Null;
        assert!(as_u32(&v).is_err());
    }
}
