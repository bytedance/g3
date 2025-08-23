/*
 * SPDX-License-Identifier: Apache-2.0
 * Copyright 2025 ByteDance and/or its affiliates.
 */

use anyhow::anyhow;
use regex::Regex;
use yaml_rust::Yaml;

pub fn as_regex(value: &Yaml) -> anyhow::Result<Regex> {
    if let Yaml::String(s) = value {
        let regex = Regex::new(s).map_err(|e| anyhow!("invalid regex value: {e}"))?;
        Ok(regex)
    } else {
        Err(anyhow!(
            "the yaml value type for regex string should be 'string'"
        ))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn as_regex_ok() {
        // valid regex string
        let yaml = yaml_str!("^\\d{3}-\\d{2}-\\d{4}$");
        assert_eq!(as_regex(&yaml).unwrap().as_str(), "^\\d{3}-\\d{2}-\\d{4}$");

        let yaml = yaml_str!("^[a-zA-Z]+$");
        assert_eq!(as_regex(&yaml).unwrap().as_str(), "^[a-zA-Z]+$");
    }

    #[test]
    fn as_regex_err() {
        // invalid regex string
        let yaml = yaml_str!("^\\d{3-\\d{2}-\\d{4}$");
        assert!(as_regex(&yaml).is_err());

        // non-string type
        let yaml = Yaml::Integer(123);
        assert!(as_regex(&yaml).is_err());
    }
}
