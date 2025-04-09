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

use g3_daemon::server::BaseServer;
use g3_types::metrics::NodeName;

use super::AggregateHandle;
use crate::collect::{ArcCollector, Collector, CollectorInternal, CollectorRegistry};
use crate::config::collector::aggregate::AggregateCollectorConfig;
use crate::config::collector::{AnyCollectorConfig, CollectorConfig};
use crate::types::MetricRecord;

pub(crate) struct AggregateCollector {
    config: Arc<AggregateCollectorConfig>,
    handle: Arc<AggregateHandle>,

    reload_sender: broadcast::Sender<Arc<AggregateCollectorConfig>>,
    reload_version: usize,
}

impl AggregateCollector {
    fn new(
        config: Arc<AggregateCollectorConfig>,
        handle: Arc<AggregateHandle>,
        reload_sender: broadcast::Sender<Arc<AggregateCollectorConfig>>,
        reload_version: usize,
    ) -> Self {
        AggregateCollector {
            config,
            handle,
            reload_sender,
            reload_version,
        }
    }

    pub(crate) fn prepare_initial(
        config: AggregateCollectorConfig,
    ) -> anyhow::Result<ArcCollector> {
        let config = Arc::new(config);
        let reload_sender = broadcast::Sender::new(4);
        let handle = AggregateHandle::spawn_new(config.clone(), reload_sender.subscribe());
        let server = AggregateCollector::new(config, handle, reload_sender, 1);
        Ok(Arc::new(server))
    }

    fn prepare_reload(&self, config: AnyCollectorConfig) -> anyhow::Result<AggregateCollector> {
        if let AnyCollectorConfig::Aggregate(config) = config {
            Ok(AggregateCollector::new(
                Arc::new(config),
                self.handle.clone(),
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

impl CollectorInternal for AggregateCollector {
    fn _clone_config(&self) -> AnyCollectorConfig {
        AnyCollectorConfig::Aggregate(self.config.as_ref().clone())
    }

    fn _depend_on_collector(&self, _name: &NodeName) -> bool {
        false
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

impl BaseServer for AggregateCollector {
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
        self.reload_version
    }
}

impl Collector for AggregateCollector {
    fn add_metric(&self, record: MetricRecord, worker_id: Option<usize>) {
        self.handle.add_metric(record, worker_id);
    }
}
