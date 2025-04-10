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

use g3_types::metrics::NodeName;

use super::{ArcCollector, ArcCollectorInternal, Collector, CollectorInternal, CollectorRegistry};
use crate::config::collector::regulate::RegulateCollectorConfig;
use crate::config::collector::{AnyCollectorConfig, CollectorConfig};
use crate::export::ArcExporter;
use crate::types::MetricRecord;

pub(crate) struct RegulateCollector {
    config: RegulateCollectorConfig,
    next: Option<ArcCollector>,
    exporters: Vec<ArcExporter>,
}

impl RegulateCollector {
    fn new<F>(config: RegulateCollectorConfig, fetch_collector: F) -> Self
    where
        F: FnMut(&NodeName) -> ArcCollector,
    {
        let next = config.next.as_ref().map(fetch_collector);
        let exporters = config
            .exporters
            .iter()
            .map(crate::export::get_or_insert_default)
            .collect();

        RegulateCollector {
            config,
            next,
            exporters,
        }
    }

    pub(crate) fn prepare_initial(
        config: RegulateCollectorConfig,
    ) -> anyhow::Result<ArcCollectorInternal> {
        let server = RegulateCollector::new(config, crate::collect::get_or_insert_default);
        Ok(Arc::new(server))
    }

    fn prepare_reload(
        &self,
        config: AnyCollectorConfig,
        registry: &mut CollectorRegistry,
    ) -> anyhow::Result<ArcCollectorInternal> {
        if let AnyCollectorConfig::Regulate(config) = config {
            Ok(Arc::new(RegulateCollector::new(config, |name| {
                registry.get_or_insert_default(name)
            })))
        } else {
            Err(anyhow!(
                "config type mismatch: expect {}, actual {}",
                self.config.collector_type(),
                config.collector_type()
            ))
        }
    }
}

impl Collector for RegulateCollector {
    #[inline]
    fn name(&self) -> &NodeName {
        self.config.name()
    }

    #[inline]
    fn r#type(&self) -> &'static str {
        self.config.collector_type()
    }

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

        for exporter in &self.exporters {
            exporter.add_metric(&record);
        }

        if let Some(next) = &self.next {
            next.add_metric(record, worker_id);
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

    fn _depend_on_exporter(&self, name: &NodeName) -> bool {
        self.config.exporters.contains(name)
    }

    fn _reload(
        &self,
        config: AnyCollectorConfig,
        registry: &mut CollectorRegistry,
    ) -> anyhow::Result<ArcCollectorInternal> {
        self.prepare_reload(config, registry)
    }
}
