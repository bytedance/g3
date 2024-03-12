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

use std::time::Duration;

use anyhow::{anyhow, Context};
use http::uri::PathAndQuery;
use yaml_rust::{yaml, Yaml};

use g3_types::metrics::StaticMetricsTags;
use g3_types::net::UpstreamAddr;

pub struct RegisterConfig {
    pub(crate) upstream: UpstreamAddr,
    pub(crate) register_path: PathAndQuery,
    pub(crate) ping_path: PathAndQuery,
    pub(crate) ping_interval: Duration,
    pub(crate) extra_data: StaticMetricsTags,
}

impl Default for RegisterConfig {
    fn default() -> Self {
        RegisterConfig {
            upstream: UpstreamAddr::empty(),
            register_path: PathAndQuery::from_static("/register"),
            ping_path: PathAndQuery::from_static("/ping"),
            ping_interval: Duration::from_secs(60),
            extra_data: StaticMetricsTags::default(),
        }
    }
}

impl RegisterConfig {
    pub(crate) fn parse(&mut self, v: &Yaml) -> anyhow::Result<()> {
        match v {
            Yaml::Hash(map) => self.parse_map(map),
            Yaml::String(_) => {
                self.upstream = g3_yaml::value::as_upstream_addr(v, 0)
                    .context("invalid upstream address string value")?;
                Ok(())
            }
            _ => Err(anyhow!("invalid yaml value type")),
        }
    }

    fn parse_map(&mut self, map: &yaml::Hash) -> anyhow::Result<()> {
        g3_yaml::foreach_kv(map, |k, v| match g3_yaml::key::normalize(k).as_str() {
            "upstream" => {
                self.upstream = g3_yaml::value::as_upstream_addr(v, 0)
                    .context(format!("invalid upstream address value for key {k}"))?;
                Ok(())
            }
            "register_path" => {
                self.register_path = g3_yaml::value::as_http_path_and_query(v)
                    .context(format!("invalid http path_query value for key {k}"))?;
                Ok(())
            }
            "ping_path" => {
                self.ping_path = g3_yaml::value::as_http_path_and_query(v)
                    .context(format!("invalid http path_query value for key {k}"))?;
                Ok(())
            }
            "ping_interval" => {
                self.ping_interval = g3_yaml::humanize::as_duration(v)
                    .context(format!("invalid humanize duration value for key {k}"))?;
                Ok(())
            }
            "extra_data" => {
                self.extra_data = g3_yaml::value::as_static_metrics_tags(v)
                    .context(format!("invalid static metrics tags value for key {k}"))?;
                Ok(())
            }
            _ => Err(anyhow!("invalid key {k}")),
        })
    }
}
