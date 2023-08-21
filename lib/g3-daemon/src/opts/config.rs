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
use std::fs;
use std::path::{Path, PathBuf};
use std::sync::OnceLock;

use anyhow::anyhow;

static CONFIG_FILE_PATH: OnceLock<PathBuf> = OnceLock::new();
static CONFIG_DIR_PATH: OnceLock<PathBuf> = OnceLock::new();

static CONFIG_FILE_EXTENSION: OnceLock<OsString> = OnceLock::new();

fn guess_config_file(dir: &Path, program_name: &'static str) -> anyhow::Result<PathBuf> {
    const GUESS_EXT: &[&str] = &["yml", "yaml", "conf"];

    let rdir = dir
        .read_dir()
        .map_err(|e| anyhow!("failed to open {}: {e}", dir.display()))?;
    for v in rdir {
        let Ok(v) = v else {
            continue;
        };
        let path = v.path();
        for ext in GUESS_EXT {
            if path.ends_with(format!("main.{ext}")) {
                return Ok(path);
            }
            if path.ends_with(format!("{program_name}.{ext}")) {
                return Ok(path);
            }
        }
    }
    Err(anyhow!(
        "no main config file found in dir {}",
        dir.display()
    ))
}

fn validate_and_get_config_file(
    path: &Path,
    program_name: &'static str,
) -> anyhow::Result<PathBuf> {
    let metadata = fs::metadata(path)
        .map_err(|e| anyhow!("failed to get metadata of path {}: {e}", path.display()))?;

    let mut path = if metadata.is_dir() {
        guess_config_file(path, program_name)?
    } else {
        path.to_path_buf()
    };

    if !path.is_absolute() {
        let cur_dir =
            std::env::current_dir().map_err(|e| anyhow!("failed to get current dir: {e}"))?;
        path = cur_dir.join(path);
    }
    path.canonicalize()
        .map_err(|e| anyhow!("failed to canonicalize path: {e}"))
}

pub fn validate_and_set_config_file(path: &Path, program_name: &'static str) -> anyhow::Result<()> {
    let config_file = validate_and_get_config_file(path, program_name)?;

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
