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
