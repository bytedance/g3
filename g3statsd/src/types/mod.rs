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

mod name;
pub(crate) use name::MetricName;

mod tag;
pub(crate) use tag::MetricTagMap;

mod value;
pub(crate) use value::MetricValue;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum MetricType {
    Counter,
    Gauge,
}

pub(crate) struct MetricRecord {
    pub(crate) r#type: MetricType,
    pub(crate) name: Arc<MetricName>,
    pub(crate) tag_map: Arc<MetricTagMap>,
    pub(crate) value: MetricValue,
}
