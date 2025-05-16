/*
 * SPDX-License-Identifier: Apache-2.0
 * Copyright 2024-2025 ByteDance and/or its affiliates.
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
            _ => config.redis.set_by_yaml_kv(k, v, lookup_dir),
        })?;

        config.check()?;
        Ok(config)
    }
}
