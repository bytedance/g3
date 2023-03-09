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

use std::fs::File;
use std::path::{Path, PathBuf};
use std::str::FromStr;

use anyhow::anyhow;
use yaml_rust::Yaml;

use g3_types::fs::ConfigFileFormat;

pub fn as_file_path(v: &Yaml, lookup_dir: &Path, auto_create: bool) -> anyhow::Result<PathBuf> {
    if let Yaml::String(path) = v {
        let path = PathBuf::from_str(path).map_err(|e| anyhow!("invalid path: {e:?}"))?;
        let path = if path.is_absolute() {
            path
        } else {
            let mut abs_path = lookup_dir.to_path_buf();
            abs_path.push(path);
            abs_path
        };
        if path.exists() {
            if !path.is_file() {
                return Err(anyhow!("the path is existed but not a regular file"));
            }
        } else if auto_create {
            if let Some(dir_path) = path.parent() {
                std::fs::create_dir_all(dir_path).map_err(|e| {
                    anyhow!("failed to create parent dir {}: {e:?}", dir_path.display(),)
                })?;
                let _ = File::create(&path)
                    .map_err(|e| anyhow!("failed to create file {}: {e:?}", path.display()))?;
            } else {
                return Err(anyhow!("the path has no valid parent dir"));
            }
        } else {
            return Err(anyhow!("path {} is not existed", path.display()));
        }
        let path = path
            .canonicalize()
            .map_err(|e| anyhow!("invalid path {}: {e:?}", path.display()))?;
        Ok(path)
    } else {
        Err(anyhow!("yaml value type for path should be string"))
    }
}

pub fn as_file(v: &Yaml, lookup_dir: Option<&Path>) -> anyhow::Result<(File, PathBuf)> {
    let path = if let Some(dir) = lookup_dir {
        as_file_path(v, dir, false)?
    } else {
        as_absolute_path(v)?
    };
    let file =
        File::open(&path).map_err(|e| anyhow!("failed to open file({}): {e:?}", path.display()))?;
    Ok((file, path))
}

pub fn as_absolute_path(v: &Yaml) -> anyhow::Result<PathBuf> {
    if let Yaml::String(path) = v {
        let path = PathBuf::from_str(path).map_err(|e| anyhow!("invalid path: {e:?}"))?;
        if path.is_relative() {
            return Err(anyhow!(
                "invalid value: {} is not an absolute path",
                path.display()
            ));
        }
        Ok(path)
    } else {
        Err(anyhow!(
            "yaml value type for absolute path should be string"
        ))
    }
}

pub fn as_config_file_format(v: &Yaml) -> anyhow::Result<ConfigFileFormat> {
    if let Yaml::String(s) = v {
        Ok(ConfigFileFormat::from_str(s)
            .map_err(|_| anyhow!("invalid config file format string"))?)
    } else {
        Err(anyhow!(
            "yaml value type for config file format should be string"
        ))
    }
}
