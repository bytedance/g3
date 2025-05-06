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
use chrono::{DateTime, Utc};

use g3_types::metrics::NodeName;

use super::{ArcExporterInternal, Exporter, ExporterInternal};
use crate::config::exporter::discard::DiscardExporterConfig;
use crate::config::exporter::{AnyExporterConfig, ExporterConfig};
use crate::types::MetricRecord;

pub(crate) struct DiscardExporter {
    config: DiscardExporterConfig,
}

impl DiscardExporter {
    fn new(config: DiscardExporterConfig) -> Self {
        DiscardExporter { config }
    }

    pub(crate) fn prepare_initial(config: DiscardExporterConfig) -> ArcExporterInternal {
        let server = DiscardExporter::new(config);
        Arc::new(server)
    }

    pub(crate) fn prepare_default(name: &NodeName) -> ArcExporterInternal {
        let config = DiscardExporterConfig::with_name(name, None);
        Arc::new(DiscardExporter::new(config))
    }

    fn prepare_reload(&self, config: AnyExporterConfig) -> anyhow::Result<DiscardExporter> {
        if let AnyExporterConfig::Discard(config) = config {
            Ok(DiscardExporter::new(config))
        } else {
            Err(anyhow!(
                "config type mismatch: expect {}, actual {}",
                self.config.exporter_type(),
                config.exporter_type()
            ))
        }
    }
}

impl Exporter for DiscardExporter {
    #[inline]
    fn name(&self) -> &NodeName {
        self.config.name()
    }

    #[inline]
    fn r#type(&self) -> &'static str {
        self.config.exporter_type()
    }

    fn add_metric(&self, _time: DateTime<Utc>, _record: &MetricRecord) {}
}

impl ExporterInternal for DiscardExporter {
    fn _clone_config(&self) -> AnyExporterConfig {
        AnyExporterConfig::Discard(self.config.clone())
    }

    fn _reload(&self, config: AnyExporterConfig) -> anyhow::Result<ArcExporterInternal> {
        let exporter = self.prepare_reload(config)?;
        Ok(Arc::new(exporter))
    }
}
