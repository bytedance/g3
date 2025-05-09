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

use std::time::Duration;

use http::HeaderValue;
use http::uri::PathAndQuery;

use g3_types::metrics::MetricTagMap;

use super::{AnyExporterConfig, ExporterConfig, ExporterConfigDiffAction};
use super::{CONFIG_KEY_EXPORTER_NAME, CONFIG_KEY_EXPORTER_TYPE};
use crate::types::MetricName;

mod precision;
pub(crate) use precision::TimestampPrecision;

mod v2;
pub(crate) use v2::InfluxdbV2ExporterConfig;

mod v3;
pub(crate) use v3::InfluxdbV3ExporterConfig;

pub(crate) trait InfluxdbExporterConfig {
    fn emit_interval(&self) -> Duration;
    fn precision(&self) -> TimestampPrecision;
    fn max_body_lines(&self) -> usize;
    fn prefix(&self) -> Option<MetricName>;
    fn global_tags(&self) -> MetricTagMap;
    fn build_api_path(&self) -> anyhow::Result<PathAndQuery>;
    fn build_api_token(&self) -> Option<HeaderValue>;
}
