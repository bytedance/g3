/*
 * SPDX-License-Identifier: Apache-2.0
 * Copyright 2023-2025 ByteDance and/or its affiliates.
 */

use std::str::FromStr;

#[derive(Clone)]
pub enum ConfigFileFormat {
    Yaml,
    Json,
}

impl FromStr for ConfigFileFormat {
    type Err = ();

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        match s.to_lowercase().as_str() {
            "json" => Ok(ConfigFileFormat::Json),
            "yaml" | "yml" => Ok(ConfigFileFormat::Yaml),
            _ => Err(()),
        }
    }
}
