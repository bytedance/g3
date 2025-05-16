/*
 * SPDX-License-Identifier: Apache-2.0
 * Copyright 2023-2025 ByteDance and/or its affiliates.
 */

use std::sync::Arc;

use arc_swap::ArcSwapOption;

use g3_types::metrics::{MetricTagMap, NodeName};
use g3_types::stats::StatId;

use crate::escape::{EscaperInterfaceStats, EscaperInternalStats, EscaperStats};

pub(super) struct DummyDenyEscaperStats {
    name: NodeName,
    id: StatId,
    extra_metrics_tags: Arc<ArcSwapOption<MetricTagMap>>,
    pub(super) interface: EscaperInterfaceStats,
}

impl DummyDenyEscaperStats {
    pub(super) fn new(name: &NodeName) -> Self {
        DummyDenyEscaperStats {
            name: name.clone(),
            id: StatId::new_unique(),
            extra_metrics_tags: Arc::new(ArcSwapOption::new(None)),
            interface: EscaperInterfaceStats::default(),
        }
    }

    pub(super) fn set_extra_tags(&self, tags: Option<Arc<MetricTagMap>>) {
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
    fn name(&self) -> &NodeName {
        &self.name
    }

    fn stat_id(&self) -> StatId {
        self.id
    }

    fn load_extra_tags(&self) -> Option<Arc<MetricTagMap>> {
        self.extra_metrics_tags.load_full()
    }

    fn share_extra_tags(&self) -> &Arc<ArcSwapOption<MetricTagMap>> {
        &self.extra_metrics_tags
    }

    fn get_task_total(&self) -> u64 {
        self.interface.get_task_total()
    }

    fn connection_attempted(&self) -> u64 {
        0
    }

    fn connection_established(&self) -> u64 {
        0
    }
}
