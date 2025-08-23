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

use super::InternalEmitter;
use crate::collect::{ArcCollectorInternal, Collector, CollectorInternal, CollectorRegistry};
use crate::config::collector::internal::InternalCollectorConfig;
use crate::config::collector::{AnyCollectorConfig, CollectorConfig};
use crate::types::MetricRecord;

pub(crate) struct InternalCollector {
    name: NodeName,
    config: ArcSwap<InternalCollectorConfig>,

    reload_sender: broadcast::Sender<Arc<InternalCollectorConfig>>,
}

impl InternalCollector {
    pub(crate) fn prepare_initial(
        config: InternalCollectorConfig,
    ) -> anyhow::Result<ArcCollectorInternal> {
        let config = Arc::new(config);
        let collector = InternalCollector {
            name: config.name().clone(),
            config: ArcSwap::new(config.clone()),
            reload_sender: broadcast::Sender::new(4),
        };
        let emitter = InternalEmitter::new(collector.reload_sender.subscribe());
        tokio::spawn(emitter.into_running(config));
        Ok(Arc::new(collector))
    }
}

impl Collector for InternalCollector {
    #[inline]
    fn name(&self) -> &NodeName {
        &self.name
    }

    #[inline]
    fn r#type(&self) -> &'static str {
        self.config.load().collector_type()
    }

    fn add_metric(&self, _time: DateTime<Utc>, _record: MetricRecord, _worker_id: Option<usize>) {}
}

impl CollectorInternal for InternalCollector {
    fn _clone_config(&self) -> AnyCollectorConfig {
        AnyCollectorConfig::Internal(self.config.load().as_ref().clone())
    }

    fn _depend_on_collector(&self, name: &NodeName) -> bool {
        self.config
            .load()
            .next
            .as_ref()
            .map(|n| n.eq(name))
            .unwrap_or(false)
    }

    fn _depend_on_exporter(&self, name: &NodeName) -> bool {
        self.config.load().exporters.contains(name)
    }

    fn _update_config(&self, config: AnyCollectorConfig) -> anyhow::Result<()> {
        let AnyCollectorConfig::Internal(config) = config else {
            return Err(anyhow!("invalid config type for Internal collector"));
        };
        let config = Arc::new(config);
        match self.reload_sender.send(config.clone()) {
            Ok(_) => {
                self.config.store(config);
                Ok(())
            }
            Err(e) => Err(anyhow!("failed to send new config to emitter: {e}")),
        }
    }

    fn _reload(
        &self,
        _config: AnyCollectorConfig,
        _registry: &mut CollectorRegistry,
    ) -> anyhow::Result<ArcCollectorInternal> {
        Err(anyhow!("reload is not needed for Internal collector"))
    }
}
