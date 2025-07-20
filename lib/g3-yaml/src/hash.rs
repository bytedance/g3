/*
 * SPDX-License-Identifier: Apache-2.0
 * Copyright 2023-2025 ByteDance and/or its affiliates.
 */

use anyhow::{Context, anyhow};
use yaml_rust::{Yaml, yaml};

pub fn foreach_kv<F>(table: &yaml::Hash, mut f: F) -> anyhow::Result<()>
where
    F: FnMut(&str, &Yaml) -> anyhow::Result<()>,
{
    for (k, v) in table.iter() {
        if let Yaml::String(key) = k {
            f(key, v).context(format!("failed to parse value of key {key}"))?;
        } else {
            return Err(anyhow!("key in hash should be string"));
        }
    }
    Ok(())
}

pub fn get_required<'a>(map: &'a yaml::Hash, k: &str) -> anyhow::Result<&'a Yaml> {
    let key = Yaml::String(k.to_owned());
    match map.get(&key) {
        Some(v) => Ok(v),
        None => Err(anyhow!("no required key {k} found in this map")),
    }
}

pub fn get_required_str<'a>(map: &'a yaml::Hash, k: &str) -> anyhow::Result<&'a str> {
    let v = get_required(map, k)?;
    if let Yaml::String(s) = v {
        Ok(s)
    } else {
        Err(anyhow!("invalid string value for required key {k}"))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use yaml_rust::YamlLoader;

    #[test]
    fn foreach_kv_ok() {
        let yaml = yaml_doc!("a: 1\nb: 2");
        let hash = yaml.as_hash().unwrap();
        let mut result = Vec::new();
        let res = foreach_kv(hash, |k, v| {
            result.push((k.to_owned(), v.as_i64().unwrap()));
            Ok(())
        });
        assert!(res.is_ok());
        assert_eq!(result, vec![("a".to_string(), 1), ("b".to_string(), 2)]);
    }

    #[test]
    fn foreach_kv_err() {
        let yaml = yaml_doc!("123: 1");
        let hash = yaml.as_hash().unwrap();
        assert!(foreach_kv(hash, |_, _| Ok(())).is_err());

        let yaml = yaml_doc!("a: 1");
        let hash = yaml.as_hash().unwrap();
        assert!(foreach_kv(hash, |k, _| Err(anyhow!("error at {}", k))).is_err());
    }

    #[test]
    fn get_required_ok() {
        let yaml = yaml_doc!("key: value");
        let hash = yaml.as_hash().unwrap();
        assert_eq!(
            get_required(hash, "key").unwrap(),
            &Yaml::String("value".to_string())
        );
    }

    #[test]
    fn get_required_err() {
        let yaml = yaml_doc!("key: value");
        let hash = yaml.as_hash().unwrap();
        assert!(get_required(hash, "missing").is_err());
    }

    #[test]
    fn get_required_str_ok() {
        let yaml = yaml_doc!("key: value");
        let hash = yaml.as_hash().unwrap();
        assert_eq!(get_required_str(hash, "key").unwrap(), "value");
    }

    #[test]
    fn get_required_str_err() {
        let yaml = yaml_doc!("key: value");
        let hash = yaml.as_hash().unwrap();
        assert!(get_required_str(hash, "missing").is_err());

        let yaml = yaml_doc!("key: 123");
        let hash = yaml.as_hash().unwrap();
        assert!(get_required_str(hash, "key").is_err());
    }
}
