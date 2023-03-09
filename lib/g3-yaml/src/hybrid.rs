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

use anyhow::{anyhow, Context};
use yaml_rust::{yaml, Yaml};

use super::YamlDocPosition;

pub struct HybridParser {
    conf_dir: PathBuf,
    conf_extension: Option<OsString>,
}

impl HybridParser {
    pub fn new(conf_dir: &Path, conf_extension: Option<&OsStr>) -> Self {
        HybridParser {
            conf_dir: PathBuf::from(conf_dir),
            conf_extension: conf_extension.map(|v| v.to_os_string()),
        }
    }

    pub fn foreach_map<F>(&self, value: &Yaml, f: &F) -> anyhow::Result<()>
    where
        F: Fn(&yaml::Hash, Option<YamlDocPosition>) -> anyhow::Result<()>,
    {
        match value {
            Yaml::String(path) => self.load_path(path, f).context(format!(
                "value is a string {path}, which should be a valid path",
            ))?,
            Yaml::Array(seq) => self
                .load_array(seq, f)
                .context(format!("value is an array, with {} objects", seq.len()))?,
            _ => return Err(anyhow!("value should be a path or an array")),
        }
        Ok(())
    }

    fn get_final_path(&self, path: &str) -> anyhow::Result<PathBuf> {
        let path = PathBuf::from(path);
        if path.is_absolute() {
            return Ok(path);
        }
        let mut final_path = self.conf_dir.clone();
        final_path.push(path);
        Ok(final_path.canonicalize()?)
    }

    fn load_array<F>(&self, entries: &[Yaml], f: &F) -> anyhow::Result<()>
    where
        F: Fn(&yaml::Hash, Option<YamlDocPosition>) -> anyhow::Result<()>,
    {
        for (i, entry) in entries.iter().enumerate() {
            match entry {
                Yaml::String(path) => self
                    .load_path(path, f)
                    .context(format!("#{i}: failed to load path {path}"))?,
                Yaml::Hash(value) => {
                    f(value, None).context(format!("#{i}: failed to load map {value:?}"))?
                }
                _ => return Err(anyhow!("#{i}: value should be a path or a map")),
            }
        }
        Ok(())
    }

    fn load_path<F>(&self, path: &str, f: &F) -> anyhow::Result<()>
    where
        F: Fn(&yaml::Hash, Option<YamlDocPosition>) -> anyhow::Result<()>,
    {
        let path = self.get_final_path(path)?;
        if path.is_dir() {
            // NOTE symlink is followed
            self.load_dir(&path, f).context(format!(
                "failed to load conf from directory {}",
                path.display()
            ))?;
        } else if path.is_file() {
            // NOTE symlink is followed
            self.load_file(&path, f)
                .context(format!("failed to load conf from file {}", path.display()))?;
        } else {
            return Err(anyhow!(
                "path {} should be a directory or file",
                path.display()
            ));
        }
        Ok(())
    }

    fn load_dir<F>(&self, path: &Path, f: &F) -> anyhow::Result<()>
    where
        F: Fn(&yaml::Hash, Option<YamlDocPosition>) -> anyhow::Result<()>,
    {
        for d_entry in std::fs::read_dir(path)? {
            let d_entry = d_entry?;

            let file_name = d_entry.path();
            if let Some(conf_extension) = &self.conf_extension {
                let extension = match file_name.extension() {
                    Some(ext) => ext,
                    None => continue,
                };
                if extension != conf_extension {
                    continue;
                }
            }

            // this get the real file type, no following
            let file_type = d_entry.file_type()?;
            if file_type.is_file() {
                self.load_file(&file_name, f).context(format!(
                    "failed to load conf from file {}",
                    file_name.display()
                ))?;
            } else if file_type.is_symlink() {
                let real_file = file_name.canonicalize()?;
                if !real_file.is_file() {
                    continue;
                }

                self.load_file(&real_file, f).context(format!(
                    "failed to load conf from file {}, followed via symlink {}",
                    real_file.display(),
                    file_name.display()
                ))?;
            }
        }
        Ok(())
    }

    fn load_file<F>(&self, path: &Path, f: &F) -> anyhow::Result<()>
    where
        F: Fn(&yaml::Hash, Option<YamlDocPosition>) -> anyhow::Result<()>,
    {
        super::foreach_doc(path, |i, doc| match doc {
            Yaml::Hash(value) => {
                let position = YamlDocPosition {
                    path: PathBuf::from(path),
                    index: i,
                };
                f(value, Some(position)).context(format!(
                    "failed to load map in conf file {} doc {}",
                    path.display(),
                    i
                ))
            }
            _ => Err(anyhow!("doc {i} in {} should be a map", path.display())),
        })
    }
}
