/*
 * SPDX-License-Identifier: Apache-2.0
 * Copyright 2025 ByteDance and/or its affiliates.
 */

use std::sync::Arc;

use anyhow::anyhow;
use arc_swap::ArcSwap;
use chrono::{DateTime, Utc};
use tokio::sync::broadcast;

use g3_types::metrics::NodeName;

use super::AggregateHandle;
use crate::collect::{ArcCollectorInternal, Collector, CollectorInternal, CollectorRegistry};
use crate::config::collector::aggregate::AggregateCollectorConfig;
use crate::config::collector::{AnyCollectorConfig, CollectorConfig};
use crate::types::MetricRecord;

pub(crate) struct AggregateCollector {
    name: NodeName,
    config: ArcSwap<AggregateCollectorConfig>,
    handle: Arc<AggregateHandle>,

    reload_sender: broadcast::Sender<Arc<AggregateCollectorConfig>>,
}

impl AggregateCollector {
    pub(crate) fn prepare_initial(
        config: AggregateCollectorConfig,
    ) -> anyhow::Result<ArcCollectorInternal> {
        let config = Arc::new(config);
        let reload_sender = broadcast::Sender::new(4);
        let handle = AggregateHandle::spawn_new(config.clone(), reload_sender.subscribe());
        let server = AggregateCollector {
            name: config.name().clone(),
            config: ArcSwap::new(config),
            handle,
            reload_sender,
        };
        Ok(Arc::new(server))
    }
}

impl Collector for AggregateCollector {
    #[inline]
    fn name(&self) -> &NodeName {
        &self.name
    }

    #[inline]
    fn r#type(&self) -> &'static str {
        self.config.load().collector_type()
    }

    fn add_metric(&self, _time: DateTime<Utc>, record: MetricRecord, worker_id: Option<usize>) {
        self.handle.add_metric(record, worker_id);
    }
}

impl CollectorInternal for AggregateCollector {
    fn _clone_config(&self) -> AnyCollectorConfig {
        AnyCollectorConfig::Aggregate(self.config.load().as_ref().clone())
    }

    fn _depend_on_collector(&self, _name: &NodeName) -> bool {
        false
    }

    fn _depend_on_exporter(&self, name: &NodeName) -> bool {
        self.config.load().exporters.contains(name)
    }

    fn _update_config(&self, config: AnyCollectorConfig) -> anyhow::Result<()> {
        let AnyCollectorConfig::Aggregate(config) = config else {
            return Err(anyhow!("invalid config type for Aggregate collector"));
        };
        let config = Arc::new(config);
        match self.reload_sender.send(config.clone()) {
            Ok(_) => {
                self.config.store(config);
                Ok(())
            }
            Err(e) => Err(anyhow!("failed to send new config to global store: {e}")),
        }
    }

    fn _reload(
        &self,
        _config: AnyCollectorConfig,
        _registry: &mut CollectorRegistry,
    ) -> anyhow::Result<ArcCollectorInternal> {
        Err(anyhow!("reload is not needed for Aggregate collector"))
    }
}
