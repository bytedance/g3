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

use std::collections::BTreeSet;

use anyhow::anyhow;
use yaml_rust::{yaml, Yaml};

use g3_resolver::driver::fail_over::FailOverDriverStaticConfig;
use g3_resolver::ResolverRuntimeConfig;
use g3_types::metrics::MetricsName;
use g3_yaml::YamlDocPosition;

use super::{AnyResolverConfig, ResolverConfig, ResolverConfigDiffAction};

const RESOLVER_CONFIG_TYPE: &str = "fail-over";

#[derive(Clone, Eq, PartialEq)]
pub(crate) struct FailOverResolverConfig {
    position: Option<YamlDocPosition>,
    name: MetricsName,
    pub(crate) runtime: ResolverRuntimeConfig,
    pub(crate) primary: MetricsName,
    pub(crate) standby: MetricsName,
    pub(crate) static_conf: FailOverDriverStaticConfig,
}

impl FailOverResolverConfig {
    fn new(position: Option<YamlDocPosition>) -> Self {
        FailOverResolverConfig {
            name: MetricsName::default(),
            position,
            runtime: Default::default(),
            primary: MetricsName::default(),
            standby: MetricsName::default(),
            static_conf: FailOverDriverStaticConfig::default(),
        }
    }

    pub(crate) fn parse(
        map: &yaml::Hash,
        position: Option<YamlDocPosition>,
    ) -> anyhow::Result<Self> {
        let mut resolver = Self::new(position);

        g3_yaml::foreach_kv(map, |k, v| resolver.set(k, v))?;

        resolver.check()?;
        Ok(resolver)
    }

    fn set(&mut self, k: &str, v: &Yaml) -> anyhow::Result<()> {
        match g3_yaml::key::normalize(k).as_str() {
            super::CONFIG_KEY_RESOLVER_TYPE => Ok(()),
            super::CONFIG_KEY_RESOLVER_NAME => {
                self.name = g3_yaml::value::as_metrics_name(v)?;
                Ok(())
            }
            "primary" => {
                self.primary = g3_yaml::value::as_metrics_name(v)?;
                Ok(())
            }
            "standby" => {
                self.standby = g3_yaml::value::as_metrics_name(v)?;
                Ok(())
            }
            "fallback_delay" | "delay" | "fallback_timeout" | "timeout" => {
                let delay = g3_yaml::humanize::as_duration(v)?;
                self.static_conf.fallback_delay(delay);
                Ok(())
            }
            "negative_ttl" | "protective_cache_ttl" => {
                let ttl = g3_yaml::value::as_u32(v)?;
                self.static_conf.set_negative_ttl(ttl);
                Ok(())
            }
            "retry_empty_record" => {
                let retry_empty_record = g3_yaml::value::as_bool(v)?;
                self.static_conf.set_retry_empty_record(retry_empty_record);
                Ok(())
            }
            "graceful_stop_wait" => {
                self.runtime.graceful_stop_wait = g3_yaml::humanize::as_duration(v)?;
                Ok(())
            }
            "protective_query_timeout" => {
                self.runtime.protective_query_timeout = g3_yaml::humanize::as_duration(v)?;
                Ok(())
            }
            _ => Err(anyhow!("invalid key {k}")),
        }
    }

    fn check(&self) -> anyhow::Result<()> {
        if self.name.is_empty() {
            return Err(anyhow!("name is not set"));
        }
        if self.primary.is_empty() {
            return Err(anyhow!("no primary next resolver set"));
        }
        if self.standby.is_empty() {
            return Err(anyhow!("no standby next resolver set"));
        }
        if self.primary.eq(&self.standby) {
            return Err(anyhow!(
                "the primary and standby next resolver should not be the same one"
            ));
        }

        Ok(())
    }
}

impl ResolverConfig for FailOverResolverConfig {
    fn name(&self) -> &MetricsName {
        &self.name
    }

    fn position(&self) -> Option<YamlDocPosition> {
        self.position.clone()
    }

    fn resolver_type(&self) -> &'static str {
        RESOLVER_CONFIG_TYPE
    }

    fn diff_action(&self, new: &AnyResolverConfig) -> ResolverConfigDiffAction {
        let new = match new {
            AnyResolverConfig::FailOver(new) => new,
            _ => return ResolverConfigDiffAction::SpawnNew,
        };

        if self.eq(new) {
            return ResolverConfigDiffAction::NoAction;
        }

        ResolverConfigDiffAction::Update
    }

    fn dependent_resolver(&self) -> Option<BTreeSet<MetricsName>> {
        let mut set = BTreeSet::new();
        set.insert(self.primary.clone());
        set.insert(self.standby.clone());
        Some(set)
    }
}
