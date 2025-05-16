/*
 * SPDX-License-Identifier: Apache-2.0
 * Copyright 2025 ByteDance and/or its affiliates.
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
