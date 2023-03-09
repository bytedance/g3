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
use std::time::Duration;

use anyhow::{anyhow, Context};
use nix::NixPath;
use yaml_rust::{yaml, Yaml};

use g3_types::fs::ConfigFileFormat;

use super::file::UserDynamicFileSource;
use crate::config::auth::UserConfig;

#[derive(Clone)]
pub(crate) struct UserDynamicPythonSource {
    pub(crate) script_file: PathBuf,
    pub(crate) cache_file: PathBuf,
    pub(crate) fetch_timeout: Duration,
    pub(crate) report_timeout: Duration,
}

impl Default for UserDynamicPythonSource {
    fn default() -> Self {
        UserDynamicPythonSource {
            script_file: PathBuf::new(),
            cache_file: PathBuf::new(),
            fetch_timeout: Duration::from_secs(30),
            report_timeout: Duration::from_secs(15),
        }
    }
}

impl UserDynamicPythonSource {
    pub(super) fn parse_map(map: &yaml::Hash, lookup_dir: &Path) -> anyhow::Result<Self> {
        let mut config = UserDynamicPythonSource::default();

        g3_yaml::foreach_kv(map, |k, v| config.set(k, v, lookup_dir))?;

        config.check()?;
        Ok(config)
    }

    fn set(&mut self, k: &str, v: &Yaml, lookup_dir: &Path) -> anyhow::Result<()> {
        match g3_yaml::key::normalize(k).as_str() {
            super::CONFIG_KEY_SOURCE_TYPE => Ok(()),
            "script" => {
                self.script_file = g3_yaml::value::as_file_path(v, lookup_dir, false)
                    .context(format!("invalid file path value for key {k}",))?;
                Ok(())
            }
            "fetch_timeout" => {
                self.fetch_timeout = g3_yaml::humanize::as_duration(v)
                    .context(format!("invalid humanize duration value for key {k}"))?;
                Ok(())
            }
            "report_timeout" => {
                self.report_timeout = g3_yaml::humanize::as_duration(v)
                    .context(format!("invalid humanize duration value for key {k}"))?;
                Ok(())
            }
            "cache_file" => {
                self.cache_file = g3_yaml::value::as_file_path(v, lookup_dir, true)
                    .context(format!("invalid value for key {k}"))?;
                Ok(())
            }
            _ => Err(anyhow!("invalid key {}", k)),
        }
    }

    fn check(&self) -> anyhow::Result<()> {
        if self.script_file.is_empty() {
            return Err(anyhow!("no script is set"));
        }
        if self.cache_file.is_empty() {
            return Err(anyhow!("no cache file is set"));
        }

        Ok(())
    }

    pub(crate) async fn fetch_cached_records(&self) -> anyhow::Result<Vec<UserConfig>> {
        let file_source = UserDynamicFileSource {
            path: self.cache_file.clone(),
            format: ConfigFileFormat::Json,
        };
        file_source.fetch_records().await
    }
}
