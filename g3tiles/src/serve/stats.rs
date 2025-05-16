/*
 * SPDX-License-Identifier: Apache-2.0
 * Copyright 2023-2025 ByteDance and/or its affiliates.
 */

use std::sync::Arc;

use g3_types::metrics::{MetricTagMap, NodeName};
use g3_types::stats::{StatId, TcpIoSnapshot, UdpIoSnapshot};

pub(crate) trait ServerStats {
    fn name(&self) -> &NodeName;
    fn stat_id(&self) -> StatId;
    fn load_extra_tags(&self) -> Option<Arc<MetricTagMap>>;

    fn is_online(&self) -> bool;

    /// count for all connections
    fn conn_total(&self) -> u64;
    /// count for real tasks
    fn task_total(&self) -> u64;
    /// count for alive tasks
    fn alive_count(&self) -> i32;

    fn tcp_io_snapshot(&self) -> Option<TcpIoSnapshot> {
        None
    }
    fn udp_io_snapshot(&self) -> Option<UdpIoSnapshot> {
        None
    }
}

pub(crate) type ArcServerStats = Arc<dyn ServerStats + Send + Sync>;
