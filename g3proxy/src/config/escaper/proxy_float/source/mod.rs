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

use std::sync::Arc;

use anyhow::anyhow;
use url::Url;
use yaml_rust::Yaml;

pub(crate) mod redis;
pub(crate) mod redis_cluster;

const CONFIG_KEY_SOURCE_TYPE: &str = "type";

#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) enum ProxyFloatSource {
    Passive,
    Redis(Arc<redis::ProxyFloatRedisSource>),
    RedisCluster(Arc<redis_cluster::ProxyFloatRedisClusterSource>),
}

impl ProxyFloatSource {
    pub(super) fn need_local_cache(&self) -> bool {
        match self {
            ProxyFloatSource::Passive => true,
            ProxyFloatSource::Redis(_) => true,
            ProxyFloatSource::RedisCluster(_) => true,
        }
    }

    pub(super) fn parse(v: &Yaml) -> anyhow::Result<Self> {
        match v {
            Yaml::Hash(map) => {
                let source_type = g3_yaml::hash_get_required_str(map, CONFIG_KEY_SOURCE_TYPE)?;

                match g3_yaml::key::normalize(source_type).as_str() {
                    "passive" => Ok(ProxyFloatSource::Passive),
                    "redis" => {
                        let source = redis::ProxyFloatRedisSource::parse_map(map)?;
                        Ok(ProxyFloatSource::Redis(Arc::new(source)))
                    }
                    "redis_cluster" | "rediscluster" => {
                        let source = redis_cluster::ProxyFloatRedisClusterSource::parse_map(map)?;
                        Ok(ProxyFloatSource::RedisCluster(Arc::new(source)))
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
                        let source = redis::ProxyFloatRedisSource::parse_url(&url)?;
                        Ok(ProxyFloatSource::Redis(Arc::new(source)))
                    }
                    _ => Err(anyhow!("unsupported url scheme: {scheme}")),
                }
            }
            Yaml::Null => Ok(ProxyFloatSource::Passive),
            _ => Err(anyhow!("invalid value type for source")),
        }
    }
}
