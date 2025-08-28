/*
 * SPDX-License-Identifier: Apache-2.0
 * Copyright 2023-2025 ByteDance and/or its affiliates.
 */

use std::path::{Path, PathBuf};
use std::str::FromStr;

use anyhow::{Context, anyhow};
use url::Url;
use yaml_rust::{Yaml, yaml};

use g3_types::fs::ConfigFileFormat;

use crate::config::auth::UserConfig;

const CONFIG_KEY_SOURCE_PATH: &str = "path";

#[derive(Clone)]
pub(crate) struct UserDynamicFileSource {
    pub(crate) path: PathBuf,
    pub(crate) format: ConfigFileFormat,
}

impl UserDynamicFileSource {
    fn new(path: PathBuf) -> Self {
        let mut format = ConfigFileFormat::Yaml;
        if let Some(extension) = path.extension()
            && let Some(s) = extension.to_str()
        {
            format = ConfigFileFormat::from_str(s).unwrap_or(format);
        }

        UserDynamicFileSource { path, format }
    }

    pub(super) fn parse_map(map: &yaml::Hash, lookup_dir: &Path) -> anyhow::Result<Self> {
        let v = g3_yaml::hash_get_required(map, CONFIG_KEY_SOURCE_PATH)?;
        let path = g3_yaml::value::as_file_path(v, lookup_dir, false).context(format!(
            "invalid path value for key {CONFIG_KEY_SOURCE_PATH}"
        ))?;
        let mut config = UserDynamicFileSource::new(path);

        g3_yaml::foreach_kv(map, |k, v| {
            config.set(k, v).context(format!("failed to parse key {k}"))
        })?;

        Ok(config)
    }

    pub(super) fn parse_url(url: &Url) -> anyhow::Result<Self> {
        let path = PathBuf::from_str(url.path())
            .map_err(|e| anyhow!("invalid file path in url: {e:?}"))?;
        let mut config = UserDynamicFileSource::new(path);

        for (k, v) in url.query_pairs() {
            let yaml_value = Yaml::String(v.to_string());
            config
                .set(&k, &yaml_value)
                .context(format!("failed to parse query param {k}={v}"))?;
        }

        Ok(config)
    }

    fn set(&mut self, k: &str, v: &Yaml) -> anyhow::Result<()> {
        match g3_yaml::key::normalize(k).as_str() {
            super::CONFIG_KEY_SOURCE_TYPE => Ok(()),
            CONFIG_KEY_SOURCE_PATH => Ok(()),
            "format" => {
                self.format = g3_yaml::value::as_config_file_format(v)
                    .context(format!("invalid config file format value for key {k}"))?;
                Ok(())
            }
            _ => Err(anyhow!("invalid key {k}")),
        }
    }

    pub(crate) async fn fetch_records(&self) -> anyhow::Result<Vec<UserConfig>> {
        // TODO limit the read size
        let contents = tokio::fs::read_to_string(&self.path)
            .await
            .map_err(|e| anyhow!("failed to read in file {}: {e}", self.path.display()))?;
        if contents.is_empty() {
            return Ok(Vec::new());
        }
        match self.format {
            ConfigFileFormat::Yaml => {
                let docs = yaml_rust::YamlLoader::load_from_str(&contents)
                    .map_err(|e| anyhow!("invalid yaml file {}: {e}", self.path.display()))?;
                UserConfig::parse_yaml_many(&docs)
            }
            ConfigFileFormat::Json => {
                let doc = serde_json::Value::from_str(&contents)
                    .map_err(|e| anyhow!("invalid json file {}: {e}", self.path.display()))?;
                UserConfig::parse_json_many(&doc)
            }
        }
    }
}
