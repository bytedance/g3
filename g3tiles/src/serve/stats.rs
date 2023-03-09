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

use g3_types::metrics::StaticMetricsTags;
use g3_types::stats::{StatId, TcpIoSnapshot, UdpIoSnapshot};

pub(crate) trait ServerStats {
    fn name(&self) -> &str;
    fn stat_id(&self) -> StatId;
    fn extra_tags(&self) -> &Arc<ArcSwapOption<StaticMetricsTags>>;

    fn is_online(&self) -> bool;

    /// count for all connections
    fn get_conn_total(&self) -> u64;
    /// count for real tasks
    fn get_task_total(&self) -> u64;
    /// count for alive tasks
    fn get_alive_count(&self) -> i32;

    fn tcp_io_snapshot(&self) -> Option<TcpIoSnapshot> {
        None
    }
    fn udp_io_snapshot(&self) -> Option<UdpIoSnapshot> {
        None
    }
}

pub(crate) type ArcServerStats = Arc<dyn ServerStats + Send + Sync>;
