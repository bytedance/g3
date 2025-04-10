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

use std::sync::Arc;
use std::time::Duration;

use anyhow::{Context, anyhow};
use ascii::AsciiString;
use yaml_rust::{Yaml, yaml};

use g3_types::acl::AclNetworkRuleBuilder;
use g3_types::metrics::{NodeName, StaticMetricsTags};
use g3_yaml::YamlDocPosition;

use super::{
    IDLE_CHECK_DEFAULT_DURATION, IDLE_CHECK_DEFAULT_MAX_COUNT, IDLE_CHECK_MAXIMUM_DURATION,
    ServerConfig,
};
use crate::config::server::{AnyServerConfig, ServerConfigDiffAction};

const SERVER_CONFIG_TYPE: &str = "KeylessProxy";

#[derive(Clone, Debug, PartialEq)]
pub(crate) struct KeylessProxyServerConfig {
    name: NodeName,
    position: Option<YamlDocPosition>,
    pub(crate) shared_logger: Option<AsciiString>,
    pub(crate) ingress_net_filter: Option<AclNetworkRuleBuilder>,
    pub(crate) extra_metrics_tags: Option<Arc<StaticMetricsTags>>,
    pub(crate) task_idle_check_duration: Duration,
    pub(crate) task_idle_max_count: usize,
    pub(crate) spawn_task_unconstrained: bool,
    pub(crate) backend: NodeName,
}

impl KeylessProxyServerConfig {
    pub(crate) fn new(position: Option<YamlDocPosition>) -> Self {
        KeylessProxyServerConfig {
            name: NodeName::default(),
            position,
            shared_logger: None,
            ingress_net_filter: None,
            extra_metrics_tags: None,
            task_idle_check_duration: IDLE_CHECK_DEFAULT_DURATION,
            task_idle_max_count: IDLE_CHECK_DEFAULT_MAX_COUNT,
            spawn_task_unconstrained: false,
            backend: NodeName::default(),
        }
    }

    pub(super) fn parse(
        map: &yaml::Hash,
        position: Option<YamlDocPosition>,
    ) -> anyhow::Result<Self> {
        let mut server = KeylessProxyServerConfig::new(position);

        g3_yaml::foreach_kv(map, |k, v| server.set(k, v))?;

        server.check()?;
        Ok(server)
    }

    fn check(&mut self) -> anyhow::Result<()> {
        if self.name.is_empty() {
            return Err(anyhow!("name is not set"));
        }
        if self.backend.is_empty() {
            return Err(anyhow!("no backend is set"));
        }
        if self.task_idle_check_duration > IDLE_CHECK_MAXIMUM_DURATION {
            self.task_idle_check_duration = IDLE_CHECK_MAXIMUM_DURATION;
        }
        Ok(())
    }

    fn set(&mut self, k: &str, v: &Yaml) -> anyhow::Result<()> {
        match g3_yaml::key::normalize(k).as_str() {
            super::CONFIG_KEY_SERVER_TYPE => Ok(()),
            super::CONFIG_KEY_SERVER_NAME => {
                self.name = g3_yaml::value::as_metric_node_name(v)?;
                Ok(())
            }
            "shared_logger" => {
                let name = g3_yaml::value::as_ascii(v)?;
                self.shared_logger = Some(name);
                Ok(())
            }
            "extra_metrics_tags" => {
                let tags = g3_yaml::value::as_static_metrics_tags(v)
                    .context(format!("invalid static metrics tags value for key {k}"))?;
                self.extra_metrics_tags = Some(Arc::new(tags));
                Ok(())
            }
            "ingress_network_filter" | "ingress_net_filter" => {
                let filter = g3_yaml::value::acl::as_ingress_network_rule_builder(v).context(
                    format!("invalid ingress network acl rule value for key {k}"),
                )?;
                self.ingress_net_filter = Some(filter);
                Ok(())
            }
            "task_idle_check_duration" => {
                self.task_idle_check_duration = g3_yaml::humanize::as_duration(v)
                    .context(format!("invalid humanize duration value for key {k}"))?;
                Ok(())
            }
            "task_idle_max_count" => {
                self.task_idle_max_count = g3_yaml::value::as_usize(v)
                    .context(format!("invalid usize value for key {k}"))?;
                Ok(())
            }
            "spawn_task_unconstrained" | "task_unconstrained" => {
                self.spawn_task_unconstrained = g3_yaml::value::as_bool(v)?;
                Ok(())
            }
            "backend" => {
                self.backend = g3_yaml::value::as_metric_node_name(v)?;
                Ok(())
            }
            _ => Err(anyhow!("invalid key {k}")),
        }
    }
}

impl ServerConfig for KeylessProxyServerConfig {
    fn name(&self) -> &NodeName {
        &self.name
    }

    fn position(&self) -> Option<YamlDocPosition> {
        self.position.clone()
    }

    fn r#type(&self) -> &'static str {
        SERVER_CONFIG_TYPE
    }

    fn diff_action(&self, new: &AnyServerConfig) -> ServerConfigDiffAction {
        let new = match new {
            AnyServerConfig::KeylessProxy(config) => config,
            _ => return ServerConfigDiffAction::SpawnNew,
        };

        if self.eq(new) {
            return ServerConfigDiffAction::NoAction;
        }

        ServerConfigDiffAction::ReloadNoRespawn
    }
}
