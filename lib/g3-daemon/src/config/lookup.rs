/*
 * SPDX-License-Identifier: Apache-2.0
 * Copyright 2023-2025 ByteDance and/or its affiliates.
 */

use std::path::Path;

use anyhow::anyhow;

use g3_yaml::YamlDocPosition;

pub fn get_lookup_dir(position: Option<&YamlDocPosition>) -> anyhow::Result<&Path> {
    if let Some(position) = position
        && let Some(dir) = position.path.parent()
    {
        return Ok(dir);
    }
    crate::opts::config_dir().ok_or_else(|| anyhow!("no valid config dir has been set"))
}
