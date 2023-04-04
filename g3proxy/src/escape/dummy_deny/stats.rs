/*
 * Copyright 2023 ByteDance and/or its affiliates.
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

use arc_swap::ArcSwapOption;

use g3_types::metrics::{MetricsName, StaticMetricsTags};
use g3_types::stats::StatId;

use crate::escape::{EscaperInterfaceStats, EscaperInternalStats, EscaperStats};

pub(super) struct DummyDenyEscaperStats {
    name: MetricsName,
    id: StatId,
    extra_metrics_tags: Arc<ArcSwapOption<StaticMetricsTags>>,
    pub(super) interface: EscaperInterfaceStats,
}

impl DummyDenyEscaperStats {
    pub(super) fn new(name: &MetricsName) -> Self {
        DummyDenyEscaperStats {
            name: name.clone(),
            id: StatId::new(),
            extra_metrics_tags: Arc::new(ArcSwapOption::new(None)),
            interface: EscaperInterfaceStats::default(),
        }
    }

    pub(super) fn set_extra_tags(&self, tags: Option<Arc<StaticMetricsTags>>) {
        self.extra_metrics_tags.store(tags);
    }
}

impl EscaperInternalStats for DummyDenyEscaperStats {
    #[inline]
    fn add_http_forward_request_attempted(&self) {
        self.interface.add_http_forward_request_attempted();
    }

    #[inline]
    fn add_https_forward_request_attempted(&self) {
        self.interface.add_https_forward_request_attempted();
    }
}

impl EscaperStats for DummyDenyEscaperStats {
    fn name(&self) -> &MetricsName {
        &self.name
    }

    fn stat_id(&self) -> StatId {
        self.id
    }

    fn extra_tags(&self) -> &Arc<ArcSwapOption<StaticMetricsTags>> {
        &self.extra_metrics_tags
    }

    fn get_task_total(&self) -> u64 {
        self.interface.get_task_total()
    }

    fn get_conn_attempted(&self) -> u64 {
        0
    }

    fn get_conn_established(&self) -> u64 {
        0
    }
}
