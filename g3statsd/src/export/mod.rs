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

use chrono::{DateTime, Utc};

use g3_types::metrics::NodeName;

use crate::config::exporter::AnyExporterConfig;
use crate::types::MetricRecord;

mod registry;
pub(crate) use registry::{get_names, get_or_insert_default};

mod ops;
pub use ops::load_all;
pub(crate) use ops::reload;

mod console;
mod discard;
mod memory;

pub(crate) trait Exporter {
    fn name(&self) -> &NodeName;
    #[allow(unused)]
    fn r#type(&self) -> &str;

    fn add_metric(&self, time: DateTime<Utc>, record: &MetricRecord);
}

trait ExporterInternal: Exporter {
    fn _clone_config(&self) -> AnyExporterConfig;

    fn _reload(&self, config: AnyExporterConfig) -> anyhow::Result<ArcExporterInternal>;

    fn _clean_to_offline(&self) {}
}

pub(crate) type ArcExporter = Arc<dyn Exporter + Send + Sync>;
type ArcExporterInternal = Arc<dyn ExporterInternal + Send + Sync>;
