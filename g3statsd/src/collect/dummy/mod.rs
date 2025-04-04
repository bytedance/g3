/*
 * Copyright 2025 ByteDance and/or its affiliates.
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

use g3_daemon::server::BaseServer;
use g3_types::metrics::NodeName;

use super::{ArcCollector, Collector, CollectorInternal};
use crate::config::collector::dummy::DummyCollectorConfig;
use crate::config::collector::{AnyCollectorConfig, CollectorConfig};

pub(crate) struct DummyCollector {
    config: DummyCollectorConfig,
}

impl DummyCollector {
    fn new(config: DummyCollectorConfig) -> Self {
        DummyCollector { config }
    }

    pub(crate) fn prepare_initial(config: DummyCollectorConfig) -> anyhow::Result<ArcCollector> {
        let server = DummyCollector::new(config);
        Ok(Arc::new(server))
    }

    pub(crate) fn prepare_default(name: &NodeName) -> ArcCollector {
        let config = DummyCollectorConfig::with_name(name, None);
        Arc::new(DummyCollector::new(config))
    }

    fn prepare_reload(&self, config: AnyCollectorConfig) -> anyhow::Result<DummyCollector> {
        if let AnyCollectorConfig::Dummy(config) = config {
            Ok(DummyCollector::new(config))
        } else {
            Err(anyhow!(
                "config type mismatch: expect {}, actual {}",
                self.config.collector_type(),
                config.collector_type()
            ))
        }
    }
}

impl CollectorInternal for DummyCollector {
    fn _clone_config(&self) -> AnyCollectorConfig {
        AnyCollectorConfig::Dummy(self.config.clone())
    }

    fn _depend_on_collector(&self, _name: &NodeName) -> bool {
        false
    }

    fn _reload_config_notify_runtime(&self) {}

    fn _update_next_collectors_in_place(&self) {}

    fn _reload_with_old_notifier(
        &self,
        config: AnyCollectorConfig,
    ) -> anyhow::Result<ArcCollector> {
        Err(anyhow!(
            "this {} collector doesn't support reload with old notifier",
            config.collector_type()
        ))
    }

    fn _reload_with_new_notifier(
        &self,
        config: AnyCollectorConfig,
    ) -> anyhow::Result<ArcCollector> {
        let server = self.prepare_reload(config)?;
        Ok(Arc::new(server))
    }

    fn _start_runtime(&self, _collector: &ArcCollector) -> anyhow::Result<()> {
        Ok(())
    }

    fn _abort_runtime(&self) {}
}

impl BaseServer for DummyCollector {
    #[inline]
    fn name(&self) -> &NodeName {
        self.config.name()
    }

    #[inline]
    fn server_type(&self) -> &'static str {
        self.config.collector_type()
    }

    #[inline]
    fn version(&self) -> usize {
        0
    }
}

impl Collector for DummyCollector {}
