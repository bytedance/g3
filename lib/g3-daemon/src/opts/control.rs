/*
 * SPDX-License-Identifier: Apache-2.0
 * Copyright 2023-2025 ByteDance and/or its affiliates.
 */

use std::path::{Path, PathBuf};
use std::sync::OnceLock;

use anyhow::anyhow;

pub const DEFAULT_CONTROL_DIR: &str = "/tmp/g3";

static CONTROL_DIR: OnceLock<PathBuf> = OnceLock::new();

pub fn validate_and_set_control_dir(path: &Path) -> anyhow::Result<()> {
    if path.is_relative() {
        return Err(anyhow!("{} is not an absolute path", path.display()));
    }

    if path.exists() && !path.is_dir() {
        return Err(anyhow!("{} is existed but not a directory", path.display()));
    }

    CONTROL_DIR
        .set(path.to_path_buf())
        .map_err(|_| anyhow!("control directory has already been set"))
}

pub fn control_dir() -> PathBuf {
    CONTROL_DIR
        .get()
        .cloned()
        .unwrap_or_else(|| PathBuf::from(DEFAULT_CONTROL_DIR))
}
