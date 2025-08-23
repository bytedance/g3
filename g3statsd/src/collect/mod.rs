/*
 * SPDX-License-Identifier: Apache-2.0
 * Copyright 2025 ByteDance and/or its affiliates.
 */

use std::sync::Arc;

use chrono::{DateTime, Utc};

use g3_types::metrics::NodeName;

use crate::config::collector::AnyCollectorConfig;
use crate::types::MetricRecord;

mod registry;
use registry::CollectorRegistry;
pub(crate) use registry::{get_names, get_or_insert_default};

mod ops;
pub use ops::load_all;
pub(crate) use ops::{reload, update_dependency_to_exporter};

mod aggregate;
mod discard;
mod internal;
mod regulate;

pub(crate) trait Collector {
    fn name(&self) -> &NodeName;
    #[allow(unused)]
    fn r#type(&self) -> &'static str;

    fn add_metric(&self, time: DateTime<Utc>, record: MetricRecord, worker_id: Option<usize>);
}

trait CollectorInternal: Collector {
    fn _clone_config(&self) -> AnyCollectorConfig;

    fn _depend_on_collector(&self, name: &NodeName) -> bool;
    fn _depend_on_exporter(&self, name: &NodeName) -> bool;

    fn _update_config(&self, _config: AnyCollectorConfig) -> anyhow::Result<()> {
        Ok(())
    }
    fn _reload(
        &self,
        config: AnyCollectorConfig,
        registry: &mut CollectorRegistry,
    ) -> anyhow::Result<ArcCollectorInternal>;

    fn _clean_to_offline(&self) {}
}

pub(crate) type ArcCollector = Arc<dyn Collector + Send + Sync>;
type ArcCollectorInternal = Arc<dyn CollectorInternal + Send + Sync>;
