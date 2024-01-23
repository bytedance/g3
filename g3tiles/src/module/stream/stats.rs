/*
 * Copyright 2024 ByteDance and/or its affiliates.
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

#![allow(unused)]

use std::sync::Arc;

use arc_swap::ArcSwapOption;

use g3_histogram::{HistogramMetricsConfig, HistogramRecorder, HistogramStats};
use g3_types::metrics::{MetricsName, StaticMetricsTags};
use g3_types::stats::StatId;

pub(crate) struct StreamBackendDurationStats {
    name: MetricsName,
    id: StatId,
    extra_metrics_tags: Arc<ArcSwapOption<StaticMetricsTags>>,

    pub(crate) general: Arc<HistogramStats>,
}

impl StreamBackendDurationStats {
    pub(crate) fn set_extra_tags(&self, tags: Option<Arc<StaticMetricsTags>>) {
        self.extra_metrics_tags.store(tags);
    }

    pub(crate) fn load_extra_tags(&self) -> Option<Arc<StaticMetricsTags>> {
        self.extra_metrics_tags.load_full()
    }

    #[inline]
    pub(crate) fn name(&self) -> &MetricsName {
        &self.name
    }

    #[inline]
    pub(crate) fn stat_id(&self) -> StatId {
        self.id
    }
}

pub(crate) struct StreamBackendDurationRecorder {
    pub(crate) general: HistogramRecorder<u64>,
}

impl StreamBackendDurationRecorder {
    pub(crate) fn new(
        name: &MetricsName,
        config: &HistogramMetricsConfig,
    ) -> (StreamBackendDurationRecorder, StreamBackendDurationStats) {
        let (general_r, general_s) =
            config.build_spawned(g3_daemon::runtime::main_handle().cloned());
        let r = StreamBackendDurationRecorder { general: general_r };
        let s = StreamBackendDurationStats {
            name: name.clone(),
            id: StatId::new(),
            extra_metrics_tags: Arc::new(ArcSwapOption::new(None)),
            general: general_s,
        };
        (r, s)
    }
}
