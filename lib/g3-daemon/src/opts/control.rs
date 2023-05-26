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

use std::path::{Path, PathBuf};

use anyhow::anyhow;
use once_cell::sync::OnceCell;

pub const DEFAULT_CONTROL_DIR: &str = "/tmp/g3";

static CONTROL_DIR: OnceCell<PathBuf> = OnceCell::new();

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
