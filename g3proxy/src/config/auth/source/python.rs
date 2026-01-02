/*
 * SPDX-License-Identifier: Apache-2.0
 * Copyright 2023-2025 ByteDance and/or its affiliates.
 */

use std::path::{Path, PathBuf};
use std::time::Duration;

use anyhow::{Context, anyhow};
use yaml_rust::{Yaml, yaml};

use g3_types::fs::ConfigFileFormat;

use super::UserDynamicFileSource;

#[derive(Clone)]
pub(crate) struct UserDynamicPythonSource {
    pub(crate) script_file: PathBuf,
    pub(crate) fetch_timeout: Duration,
    pub(crate) report_timeout: Duration,
}

impl Default for UserDynamicPythonSource {
    fn default() -> Self {
        UserDynamicPythonSource {
            script_file: PathBuf::new(),
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
                    .context(format!("invalid file path value for key {k}"))?;
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
            _ => Err(anyhow!("invalid key {}", k)),
        }
    }

    fn check(&self) -> anyhow::Result<()> {
        if self.script_file.as_os_str().is_empty() {
            return Err(anyhow!("no script is set"));
        }

        Ok(())
    }

    pub(crate) fn cache(&self, cache: &Path) -> UserDynamicFileSource {
        UserDynamicFileSource {
            path: cache.to_path_buf(),
            format: ConfigFileFormat::Json,
        }
    }
}
