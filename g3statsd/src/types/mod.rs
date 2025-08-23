/*
 * SPDX-License-Identifier: Apache-2.0
 * Copyright 2025 ByteDance and/or its affiliates.
 */

use std::sync::Arc;

use g3_types::metrics::MetricTagMap;

mod name;
pub(crate) use name::MetricName;

mod value;
pub(crate) use value::MetricValue;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum MetricType {
    Counter,
    Gauge,
}

#[derive(Clone)]
pub(crate) struct MetricRecord {
    pub(crate) r#type: MetricType,
    pub(crate) name: Arc<MetricName>,
    pub(crate) tag_map: Arc<MetricTagMap>,
    pub(crate) value: MetricValue,
}
