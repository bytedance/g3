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
use tokio::sync::broadcast;

use g3_types::metrics::NodeName;

use super::InternalEmitter;
use crate::collect::{ArcCollector, Collector, CollectorInternal, CollectorRegistry};
use crate::config::collector::internal::InternalCollectorConfig;
use crate::config::collector::{AnyCollectorConfig, CollectorConfig};
use crate::types::MetricRecord;

pub(crate) struct InternalCollector {
    config: Arc<InternalCollectorConfig>,

    reload_sender: broadcast::Sender<Arc<InternalCollectorConfig>>,
    reload_version: usize,
}

impl InternalCollector {
    fn new(
        config: InternalCollectorConfig,
        reload_sender: broadcast::Sender<Arc<InternalCollectorConfig>>,
        reload_version: usize,
    ) -> Self {
        InternalCollector {
            config: Arc::new(config),
            reload_sender,
            reload_version,
        }
    }

    pub(crate) fn prepare_initial(config: InternalCollectorConfig) -> anyhow::Result<ArcCollector> {
        let server = InternalCollector::new(config, broadcast::Sender::new(4), 1);
        let emitter = InternalEmitter::new(server.reload_sender.subscribe());
        let config = server.config.clone();
        tokio::spawn(emitter.into_running(config));
        Ok(Arc::new(server))
    }

    fn prepare_reload(&self, config: AnyCollectorConfig) -> anyhow::Result<InternalCollector> {
        if let AnyCollectorConfig::Internal(config) = config {
            Ok(InternalCollector::new(
                config,
                self.reload_sender.clone(),
                self.reload_version + 1,
            ))
        } else {
            Err(anyhow!(
                "config type mismatch: expect {}, actual {}",
                self.config.collector_type(),
                config.collector_type()
            ))
        }
    }
}

impl CollectorInternal for InternalCollector {
    fn _clone_config(&self) -> AnyCollectorConfig {
        AnyCollectorConfig::Internal((*self.config).clone())
    }

    fn _depend_on_collector(&self, name: &NodeName) -> bool {
        self.config
            .next
            .as_ref()
            .map(|n| n.eq(name))
            .unwrap_or(false)
    }

    fn _depend_on_exporter(&self, name: &NodeName) -> bool {
        self.config.exporters.contains(name)
    }

    fn _reload(
        &self,
        config: AnyCollectorConfig,
        _registry: &mut CollectorRegistry,
    ) -> anyhow::Result<ArcCollector> {
        let server = self.prepare_reload(config)?;
        let _ = self.reload_sender.send(self.config.clone());
        Ok(Arc::new(server))
    }
}

impl Collector for InternalCollector {
    #[inline]
    fn name(&self) -> &NodeName {
        self.config.name()
    }

    #[inline]
    fn collector_type(&self) -> &'static str {
        self.config.collector_type()
    }

    #[inline]
    fn version(&self) -> usize {
        self.reload_version
    }

    fn add_metric(&self, _record: MetricRecord, _worker_id: Option<usize>) {}
}
