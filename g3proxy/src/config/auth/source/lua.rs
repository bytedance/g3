/*
 * SPDX-License-Identifier: Apache-2.0
 * Copyright 2023-2025 ByteDance and/or its affiliates.
 */

use std::path::{Path, PathBuf};
use std::time::Duration;

use anyhow::{Context, anyhow};
use log::warn;
use yaml_rust::{Yaml, yaml};

use g3_types::fs::ConfigFileFormat;

use super::file::UserDynamicFileSource;
use crate::config::auth::UserConfig;

#[derive(Clone)]
pub(crate) struct UserDynamicLuaSource {
    pub(crate) fetch_script: PathBuf,
    pub(crate) fetch_timeout: Duration,
    pub(crate) report_script: Option<PathBuf>,
    pub(crate) report_timeout: Duration,
    pub(crate) cache_file: PathBuf,
}

impl Default for UserDynamicLuaSource {
    fn default() -> Self {
        UserDynamicLuaSource {
            fetch_script: PathBuf::default(),
            fetch_timeout: Duration::from_secs(30),
            report_script: None,
            report_timeout: Duration::from_secs(15),
            cache_file: PathBuf::default(),
        }
    }
}

impl UserDynamicLuaSource {
    pub(super) fn parse_map(map: &yaml::Hash, lookup_dir: &Path) -> anyhow::Result<Self> {
        let mut config = UserDynamicLuaSource::default();

        g3_yaml::foreach_kv(map, |k, v| config.set(k, v, lookup_dir))?;

        config.check()?;
        Ok(config)
    }

    fn set(&mut self, k: &str, v: &Yaml, lookup_dir: &Path) -> anyhow::Result<()> {
        match g3_yaml::key::normalize(k).as_str() {
            super::CONFIG_KEY_SOURCE_TYPE => Ok(()),
            "fetch_script" | "script" => {
                let script = g3_yaml::value::as_file_path(v, lookup_dir, false)
                    .context(format!("invalid file path value for key {k}"))?;
                self.fetch_script = script;
                Ok(())
            }
            "fetch_timeout" | "timeout" => {
                self.fetch_timeout = g3_yaml::humanize::as_duration(v)
                    .context(format!("invalid humanize duration value for key {k}"))?;
                Ok(())
            }
            "report_script" => {
                let script_file = g3_yaml::value::as_file_path(v, lookup_dir, false)
                    .context(format!("invalid file path value for key {k}"))?;
                self.report_script = Some(script_file);
                Ok(())
            }
            "report_timeout" => {
                self.report_timeout = g3_yaml::humanize::as_duration(v)
                    .context(format!("invalid humanize duration value for key {k}"))?;
                Ok(())
            }
            "cache_file" => {
                warn!(
                    "this config option is deprecated, use user-group level cache option instead"
                );
                let cache_file = g3_yaml::value::as_file_path(v, lookup_dir, true)
                    .context(format!("invalid file path value for key {k}"))?;
                self.cache_file = cache_file;
                Ok(())
            }
            _ => Err(anyhow!("invalid key {k}")),
        }
    }

    fn check(&self) -> anyhow::Result<()> {
        if self.fetch_script.as_os_str().is_empty() {
            return Err(anyhow!("no fetch script is set"));
        }

        Ok(())
    }

    pub(crate) fn real_cache_path<'a>(&'a self, cache: &'a Path) -> &'a Path {
        if cache.as_os_str().is_empty() {
            self.cache_file.as_path()
        } else {
            cache
        }
    }

    pub(crate) async fn fetch_cached_records(
        &self,
        cache: &Path,
    ) -> anyhow::Result<Vec<UserConfig>> {
        let file_source = UserDynamicFileSource {
            path: self.real_cache_path(cache).to_path_buf(),
            format: ConfigFileFormat::Json,
        };
        file_source.fetch_records().await
    }
}
