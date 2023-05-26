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

use std::ffi::{OsStr, OsString};
use std::path::{Path, PathBuf};

use anyhow::anyhow;
use once_cell::sync::OnceCell;

static CONFIG_FILE_PATH: OnceCell<PathBuf> = OnceCell::new();
static CONFIG_DIR_PATH: OnceCell<PathBuf> = OnceCell::new();

static CONFIG_FILE_EXTENSION: OnceCell<OsString> = OnceCell::new();

fn validate_and_get_config_file(path: &Path) -> anyhow::Result<PathBuf> {
    if path.is_absolute() {
        return Ok(path.to_path_buf());
    }
    let mut dir = std::env::current_dir()?;
    dir.push(path);
    dir.canonicalize()?;
    Ok(dir)
}

pub fn validate_and_set_config_file(path: &Path) -> anyhow::Result<()> {
    let config_file = validate_and_get_config_file(path)?;

    CONFIG_FILE_PATH
        .set(config_file.clone())
        .map_err(|_| anyhow!("config file has already been set"))?;

    let current_dir = std::env::current_dir()?;
    let conf_dir = config_file.parent().unwrap_or(&current_dir);
    CONFIG_DIR_PATH
        .set(conf_dir.to_path_buf())
        .map_err(|_| anyhow!("config dir has already been set"))?;

    if let Some(ext) = config_file.extension() {
        CONFIG_FILE_EXTENSION
            .set(ext.to_os_string())
            .map_err(|_| anyhow!("config file extension has already been set"))?;
    }

    Ok(())
}

pub fn config_file() -> Option<&'static Path> {
    CONFIG_FILE_PATH.get().map(|d| d.as_path())
}

pub fn config_dir() -> Option<&'static Path> {
    CONFIG_DIR_PATH.get().map(|d| d.as_path())
}

pub fn config_file_extension() -> Option<&'static OsStr> {
    CONFIG_FILE_EXTENSION.get().map(|s| s.as_os_str())
}
