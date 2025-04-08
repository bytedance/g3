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
use arc_swap::ArcSwapOption;

use g3_daemon::server::BaseServer;
use g3_types::metrics::NodeName;

use super::{ArcCollector, Collector, CollectorInternal};
use crate::config::collector::regulate::RegulateCollectorConfig;
use crate::config::collector::{AnyCollectorConfig, CollectorConfig};
use crate::types::MetricRecord;

pub(crate) struct RegulateCollector {
    config: RegulateCollectorConfig,
    next: ArcSwapOption<ArcCollector>,

    reload_version: usize,
}

impl RegulateCollector {
    fn new(config: RegulateCollectorConfig, reload_version: usize) -> Self {
        let next = config
            .next
            .as_ref()
            .map(|name| Arc::new(crate::collect::get_or_insert_default(name)));

        RegulateCollector {
            config,
            next: ArcSwapOption::new(next),
            reload_version,
        }
    }

    pub(crate) fn prepare_initial(config: RegulateCollectorConfig) -> anyhow::Result<ArcCollector> {
        let server = RegulateCollector::new(config, 0);
        Ok(Arc::new(server))
    }

    fn prepare_reload(&self, config: AnyCollectorConfig) -> anyhow::Result<ArcCollector> {
        if let AnyCollectorConfig::Regulate(config) = config {
            Ok(Arc::new(RegulateCollector::new(
                config,
                self.reload_version + 1,
            )))
        } else {
            Err(anyhow!(
                "config type mismatch: expect {}, actual {}",
                self.config.collector_type(),
                config.collector_type()
            ))
        }
    }
}

impl CollectorInternal for RegulateCollector {
    fn _clone_config(&self) -> AnyCollectorConfig {
        AnyCollectorConfig::Regulate(self.config.clone())
    }

    fn _depend_on_collector(&self, name: &NodeName) -> bool {
        self.config
            .next
            .as_ref()
            .map(|n| n.eq(name))
            .unwrap_or(false)
    }

    fn _lock_safe_reload(&self, config: AnyCollectorConfig) -> anyhow::Result<ArcCollector> {
        self.prepare_reload(config)
    }

    fn _update_next_collectors_in_place(&self) {
        if let Some(name) = &self.config.next {
            let next = crate::collect::get_or_insert_default(name);
            self.next.store(Some(Arc::new(next)));
        }
    }
}

impl BaseServer for RegulateCollector {
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

impl Collector for RegulateCollector {
    fn add_metric(&self, mut record: MetricRecord, worker_id: Option<usize>) {
        if let Some(prefix) = &self.config.prefix {
            let name = Arc::make_mut(&mut record.name);
            name.add_prefix(prefix);
        }
        if !self.config.drop_tags.is_empty() {
            let tag_map = Arc::make_mut(&mut record.tag_map);
            for tag_name in &self.config.drop_tags {
                tag_map.drop(tag_name);
            }
        }

        // TODO send to exporter

        if let Some(next) = self.next.load().as_ref() {
            next.add_metric(record, worker_id);
        }
    }
}
