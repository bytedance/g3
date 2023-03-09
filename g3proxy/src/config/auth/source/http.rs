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
use url::Url;
use yaml_rust::{yaml, Yaml};

use g3_types::fs::ConfigFileFormat;

use super::file::UserDynamicFileSource;
use crate::config::auth::UserConfig;

const CONFIG_KEY_SOURCE_URL: &str = "url";
const CONFIG_KEY_SOURCE_CACHE_FILE: &str = "cache_file";

#[derive(Clone)]
pub(crate) struct UserDynamicHttpSource {
    pub(crate) url: Url,
    pub(crate) cache_file: PathBuf,
    pub(crate) timeout: Duration,
    pub(crate) connect_timeout: Duration,
    pub(crate) interface: String,
    pub(crate) max_body_size: usize,
}

impl UserDynamicHttpSource {
    pub(super) fn new(url: Url, cache_file: PathBuf) -> Self {
        UserDynamicHttpSource {
            url,
            cache_file,
            timeout: Duration::from_secs(10),
            connect_timeout: Duration::from_secs(1),
            interface: String::new(),
            max_body_size: 64 << 20, // 64MB
        }
    }

    pub(super) fn parse_map(map: &yaml::Hash, lookup_dir: &Path) -> anyhow::Result<Self> {
        let v = g3_yaml::hash_get_required(map, CONFIG_KEY_SOURCE_URL)?;

        let url = g3_yaml::value::as_url(v)
            .context(format!("invalid url value for key {CONFIG_KEY_SOURCE_URL}"))?;

        let v = g3_yaml::hash_get_required(map, CONFIG_KEY_SOURCE_CACHE_FILE)?;

        let cache_file = g3_yaml::value::as_file_path(v, lookup_dir, true).context(format!(
            "invalid value for key {CONFIG_KEY_SOURCE_CACHE_FILE}"
        ))?;

        let mut config = UserDynamicHttpSource::new(url, cache_file);

        g3_yaml::foreach_kv(map, |k, v| config.set(k, v))?;

        Ok(config)
    }

    fn set(&mut self, k: &str, v: &Yaml) -> anyhow::Result<()> {
        match g3_yaml::key::normalize(k).as_str() {
            super::CONFIG_KEY_SOURCE_TYPE => Ok(()),
            CONFIG_KEY_SOURCE_URL => Ok(()),
            CONFIG_KEY_SOURCE_CACHE_FILE => Ok(()),
            "timeout" => {
                self.timeout = g3_yaml::humanize::as_duration(v)
                    .context(format!("invalid humanize duration value for key {k}"))?;
                Ok(())
            }
            "connect_timeout" => {
                self.connect_timeout = g3_yaml::humanize::as_duration(v)
                    .context(format!("invalid humanize duration value for key {k}"))?;
                Ok(())
            }
            "interface" | "bind" => {
                self.interface = g3_yaml::value::as_string(v)
                    .context(format!("invalid string value for key {k}"))?;
                Ok(())
            }
            "max_body_size" => {
                self.max_body_size = g3_yaml::humanize::as_usize(v)
                    .context(format!("invalid humanize size value for key {k}"))?;
                Ok(())
            }
            _ => Err(anyhow!("invalid key {k}")),
        }
    }

    pub(crate) async fn fetch_cached_records(&self) -> anyhow::Result<Vec<UserConfig>> {
        let file_source = UserDynamicFileSource {
            path: self.cache_file.clone(),
            format: ConfigFileFormat::Json,
        };
        file_source.fetch_records().await
    }
}
