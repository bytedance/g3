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

use std::path::Path;
use std::sync::Arc;

use anyhow::anyhow;
use url::Url;
use yaml_rust::Yaml;

pub(crate) mod cache;
pub(crate) mod file;

#[cfg(feature = "lua")]
pub(crate) mod lua;

#[cfg(feature = "python")]
pub(crate) mod python;

const CONFIG_KEY_SOURCE_TYPE: &str = "type";

#[derive(Clone)]
pub(crate) enum UserDynamicSource {
    File(Arc<file::UserDynamicFileSource>),
    #[cfg(feature = "lua")]
    Lua(Arc<lua::UserDynamicLuaSource>),
    #[cfg(feature = "python")]
    Python(Arc<python::UserDynamicPythonSource>),
}

impl UserDynamicSource {
    pub(super) fn parse_config(v: &Yaml, lookup_dir: &Path) -> anyhow::Result<Self> {
        match v {
            Yaml::Hash(map) => {
                let source_type = g3_yaml::hash_get_required_str(map, CONFIG_KEY_SOURCE_TYPE)?;

                match g3_yaml::key::normalize(source_type).as_str() {
                    "file" => {
                        let source = file::UserDynamicFileSource::parse_map(map, lookup_dir)?;
                        Ok(UserDynamicSource::File(Arc::new(source)))
                    }
                    #[cfg(feature = "lua")]
                    "lua" => {
                        let source = lua::UserDynamicLuaSource::parse_map(map, lookup_dir)?;
                        Ok(UserDynamicSource::Lua(Arc::new(source)))
                    }
                    #[cfg(feature = "python")]
                    "python" => {
                        let source = python::UserDynamicPythonSource::parse_map(map, lookup_dir)?;
                        Ok(UserDynamicSource::Python(Arc::new(source)))
                    }
                    _ => Err(anyhow!("unsupported source type {source_type}")),
                }
            }
            Yaml::String(url) => {
                let url = Url::parse(url)
                    .map_err(|e| anyhow!("the string value is not a valid url: {e}"))?;
                let scheme = url.scheme();
                match g3_yaml::key::normalize(scheme).as_str() {
                    "file" => {
                        let source = file::UserDynamicFileSource::parse_url(&url)?;
                        Ok(UserDynamicSource::File(Arc::new(source)))
                    }
                    _ => Err(anyhow!("unsupported url scheme: {scheme}")),
                }
            }
            _ => Err(anyhow!("invalid value type for source")),
        }
    }
}
