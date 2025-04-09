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

pub(crate) trait ExporterInternal {
    fn _clone_config(&self) -> AnyExporterConfig;

    fn _reload(&self, config: AnyExporterConfig) -> anyhow::Result<ArcExporter>;

    fn _clean_to_offline(&self) {}
}

pub(crate) trait Exporter: ExporterInternal {
    #[allow(unused)]
    fn name(&self) -> &NodeName;
    #[allow(unused)]
    fn exporter_type(&self) -> &str;

    fn add_metric(&self, record: &MetricRecord);
}

pub(crate) type ArcExporter = Arc<dyn Exporter + Send + Sync>;
