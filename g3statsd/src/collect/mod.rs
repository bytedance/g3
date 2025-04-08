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

use g3_daemon::server::BaseServer;
use g3_types::metrics::NodeName;

use crate::config::collector::AnyCollectorConfig;
use crate::types::MetricRecord;

mod registry;
pub(crate) use registry::{get_names, get_or_insert_default};

mod ops;
pub(crate) use ops::reload;
pub use ops::spawn_all;

mod aggregate;
mod discard;
mod internal;
mod regulate;

pub(crate) trait CollectorInternal {
    fn _clone_config(&self) -> AnyCollectorConfig;

    fn _depend_on_collector(&self, name: &NodeName) -> bool;

    /// registry lock is allowed in this method
    fn _lock_safe_reload(&self, config: AnyCollectorConfig) -> anyhow::Result<ArcCollector>;

    fn _clean_to_offline(&self) {}

    fn _update_next_collectors_in_place(&self);
}

pub(crate) trait Collector: CollectorInternal + BaseServer {
    fn add_metric(&self, record: MetricRecord, worker_id: Option<usize>);
}

pub(crate) type ArcCollector = Arc<dyn Collector + Send + Sync>;
