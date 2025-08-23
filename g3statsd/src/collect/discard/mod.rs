/*
 * SPDX-License-Identifier: Apache-2.0
 * Copyright 2025 ByteDance and/or its affiliates.
 */

use std::sync::Arc;

use anyhow::anyhow;
use chrono::{DateTime, Utc};

use g3_types::metrics::NodeName;

use super::{ArcCollectorInternal, Collector, CollectorInternal, CollectorRegistry};
use crate::config::collector::discard::DiscardCollectorConfig;
use crate::config::collector::{AnyCollectorConfig, CollectorConfig};
use crate::types::MetricRecord;

pub(crate) struct DiscardCollector {
    config: DiscardCollectorConfig,
}

impl DiscardCollector {
    fn new(config: DiscardCollectorConfig) -> Self {
        DiscardCollector { config }
    }

    pub(crate) fn prepare_initial(
        config: DiscardCollectorConfig,
    ) -> anyhow::Result<ArcCollectorInternal> {
        let server = DiscardCollector::new(config);
        Ok(Arc::new(server))
    }

    pub(crate) fn prepare_default(name: &NodeName) -> ArcCollectorInternal {
        let config = DiscardCollectorConfig::with_name(name, None);
        Arc::new(DiscardCollector::new(config))
    }

    fn prepare_reload(&self, config: AnyCollectorConfig) -> anyhow::Result<ArcCollectorInternal> {
        if let AnyCollectorConfig::Discard(config) = config {
            Ok(Arc::new(DiscardCollector::new(config)))
        } else {
            Err(anyhow!(
                "config type mismatch: expect {}, actual {}",
                self.config.collector_type(),
                config.collector_type()
            ))
        }
    }
}

impl Collector for DiscardCollector {
    #[inline]
    fn name(&self) -> &NodeName {
        self.config.name()
    }

    #[inline]
    fn r#type(&self) -> &'static str {
        self.config.collector_type()
    }

    fn add_metric(&self, _time: DateTime<Utc>, _record: MetricRecord, _worker_id: Option<usize>) {}
}

impl CollectorInternal for DiscardCollector {
    fn _clone_config(&self) -> AnyCollectorConfig {
        AnyCollectorConfig::Discard(self.config.clone())
    }

    fn _depend_on_collector(&self, _name: &NodeName) -> bool {
        false
    }

    fn _depend_on_exporter(&self, _name: &NodeName) -> bool {
        false
    }

    fn _reload(
        &self,
        config: AnyCollectorConfig,
        _registry: &mut CollectorRegistry,
    ) -> anyhow::Result<ArcCollectorInternal> {
        self.prepare_reload(config)
    }
}
