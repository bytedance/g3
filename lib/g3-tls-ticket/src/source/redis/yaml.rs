/*
 * Copyright 2024 ByteDance and/or its affiliates.
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

use yaml_rust::yaml;

use super::RedisSourceConfig;
use crate::source::CONFIG_KEY_SOURCE_TYPE;

impl RedisSourceConfig {
    pub(crate) fn parse_yaml_map(
        map: &yaml::Hash,
        lookup_dir: Option<&Path>,
    ) -> anyhow::Result<Self> {
        let mut config = RedisSourceConfig::default();

        g3_yaml::foreach_kv(map, |k, v| match g3_yaml::key::normalize(k).as_str() {
            CONFIG_KEY_SOURCE_TYPE => Ok(()),
            "enc_key" => {
                config.enc_key_name = g3_yaml::value::as_string(v)?;
                Ok(())
            }
            "dec_set" => {
                config.dec_set_name = g3_yaml::value::as_string(v)?;
                Ok(())
            }
            normalized_key => config.redis.set_yaml_kv(normalized_key, v, lookup_dir),
        })?;

        config.check()?;
        Ok(config)
    }
}
