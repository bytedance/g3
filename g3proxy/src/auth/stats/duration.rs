/*
 * SPDX-License-Identifier: Apache-2.0
 * Copyright 2023-2025 ByteDance and/or its affiliates.
 */

use std::sync::Arc;
use std::time::Duration;

use arc_swap::ArcSwapOption;

use g3_histogram::{HistogramMetricsConfig, HistogramRecorder, HistogramStats};
use g3_types::ext::DurationExt;
use g3_types::metrics::{MetricTagMap, NodeName};
use g3_types::stats::StatId;

use crate::auth::UserType;

pub(crate) struct UserSiteDurationRecorder {
    task_ready: HistogramRecorder<u64>,
}

impl UserSiteDurationRecorder {
    pub(crate) fn new(
        user_group: &NodeName,
        user: &str,
        user_type: UserType,
        server: &NodeName,
        server_extra_tags: &Arc<ArcSwapOption<MetricTagMap>>,
        config: &HistogramMetricsConfig,
    ) -> (Self, Arc<UserSiteDurationStats>) {
        let (task_ready_r, task_ready_s) =
            config.build_spawned(g3_daemon::runtime::main_handle().cloned());

        let stats = UserSiteDurationStats {
            id: StatId::new_unique(),
            user_group: user_group.clone(),
            user: user.to_string(),
            user_type,
            server: server.clone(),
            server_extra_tags: server_extra_tags.clone(),
            task_ready: task_ready_s,
        };
        let recorder = UserSiteDurationRecorder {
            task_ready: task_ready_r,
        };
        (recorder, Arc::new(stats))
    }

    pub(crate) fn record_task_ready(&self, dur: Duration) {
        let _ = self.task_ready.record(dur.as_nanos_u64());
    }
}

pub(crate) struct UserSiteDurationStats {
    id: StatId,
    user_group: NodeName,
    user: String,
    user_type: UserType,
    server: NodeName,
    server_extra_tags: Arc<ArcSwapOption<MetricTagMap>>,

    pub(crate) task_ready: Arc<HistogramStats>,
}

impl UserSiteDurationStats {
    #[inline]
    pub(crate) fn stat_id(&self) -> StatId {
        self.id
    }

    #[inline]
    pub(crate) fn user_group(&self) -> &NodeName {
        &self.user_group
    }

    #[inline]
    pub(crate) fn user(&self) -> &str {
        &self.user
    }

    #[inline]
    pub(crate) fn user_type(&self) -> &str {
        self.user_type.as_str()
    }

    #[inline]
    pub(crate) fn server(&self) -> &NodeName {
        &self.server
    }

    #[inline]
    pub(crate) fn server_extra_tags(&self) -> Option<Arc<MetricTagMap>> {
        self.server_extra_tags.load_full()
    }
}
