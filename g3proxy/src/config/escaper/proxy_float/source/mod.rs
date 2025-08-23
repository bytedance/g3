/*
 * SPDX-License-Identifier: Apache-2.0
 * Copyright 2023-2025 ByteDance and/or its affiliates.
 */

use anyhow::anyhow;
use url::Url;
use yaml_rust::Yaml;

use g3_yaml::YamlDocPosition;

pub(crate) mod redis;

const CONFIG_KEY_SOURCE_TYPE: &str = "type";

#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) enum ProxyFloatSource {
    Passive,
    Redis(Box<redis::ProxyFloatRedisSource>),
}

impl ProxyFloatSource {
    pub(super) fn need_local_cache(&self) -> bool {
        match self {
            ProxyFloatSource::Passive => true,
            ProxyFloatSource::Redis(_) => true,
        }
    }

    pub(super) fn parse(v: &Yaml, position: Option<&YamlDocPosition>) -> anyhow::Result<Self> {
        match v {
            Yaml::Hash(map) => {
                let source_type = g3_yaml::hash_get_required_str(map, CONFIG_KEY_SOURCE_TYPE)?;

                match g3_yaml::key::normalize(source_type).as_str() {
                    "passive" => Ok(ProxyFloatSource::Passive),
                    "redis" => {
                        let source = redis::ProxyFloatRedisSource::parse_map(map, position)?;
                        Ok(ProxyFloatSource::Redis(Box::new(source)))
                    }
                    _ => Err(anyhow!("unsupported source type {source_type}")),
                }
            }
            Yaml::String(url) => {
                let url = Url::parse(url)
                    .map_err(|e| anyhow!("the string value is not a valid url: {e}"))?;
                let scheme = url.scheme();
                match g3_yaml::key::normalize(scheme).as_str() {
                    "redis" => {
                        let source = redis::ProxyFloatRedisSource::parse_url(&url, position)?;
                        Ok(ProxyFloatSource::Redis(Box::new(source)))
                    }
                    _ => Err(anyhow!("unsupported url scheme: {scheme}")),
                }
            }
            Yaml::Null => Ok(ProxyFloatSource::Passive),
            _ => Err(anyhow!("invalid value type for source")),
        }
    }
}
