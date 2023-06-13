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

use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::Arc;

use arc_swap::ArcSwapOption;

use g3_types::metrics::{MetricsName, StaticMetricsTags};
use g3_types::stats::{StatId, TcpIoSnapshot, TcpIoStats, UdpIoSnapshot, UdpIoStats};

pub(crate) trait EscaperInternalStats {
    fn add_http_forward_request_attempted(&self);
    fn add_https_forward_request_attempted(&self);
}

pub(crate) trait EscaperStats: EscaperInternalStats {
    fn name(&self) -> &MetricsName;
    fn stat_id(&self) -> StatId;
    fn extra_tags(&self) -> &Arc<ArcSwapOption<StaticMetricsTags>>;

    /// count for tasks
    fn get_task_total(&self) -> u64;

    /// count for attempted established connections
    fn get_conn_attempted(&self) -> u64;
    fn get_conn_established(&self) -> u64;

    fn tcp_io_snapshot(&self) -> Option<TcpIoSnapshot> {
        None
    }

    fn udp_io_snapshot(&self) -> Option<UdpIoSnapshot> {
        None
    }

    fn forbidden_snapshot(&self) -> Option<EscaperForbiddenSnapshot> {
        None
    }
}

pub(crate) type ArcEscaperInternalStats = Arc<dyn EscaperInternalStats + Send + Sync>;
pub(crate) type ArcEscaperStats = Arc<dyn EscaperStats + Send + Sync>;

#[derive(Default)]
pub(crate) struct EscaperForbiddenSnapshot {
    pub(crate) ip_blocked: u64,
}

#[derive(Default)]
pub(crate) struct EscaperForbiddenStats {
    ip_blocked: AtomicU64,
}

impl EscaperForbiddenStats {
    pub(crate) fn add_ip_blocked(&self) {
        self.ip_blocked.fetch_add(1, Ordering::Relaxed);
    }

    pub(crate) fn snapshot(&self) -> EscaperForbiddenSnapshot {
        EscaperForbiddenSnapshot {
            ip_blocked: self.ip_blocked.load(Ordering::Relaxed),
        }
    }
}

#[derive(Default)]
pub(crate) struct EscaperInterfaceStats {
    tcp_connect_attempted: AtomicU64,
    tls_connect_attempted: AtomicU64,
    udp_connect_attempted: AtomicU64,
    udp_relay_session_attempted: AtomicU64,
    http_forward_request_attempted: AtomicU64,
    https_forward_request_attempted: AtomicU64,
    ftp_over_http_request_attempted: AtomicU64,
    // for http forward keepalive
    http_forward_connection_attempted: AtomicU64,
    https_forward_connection_attempted: AtomicU64,
    // for ftp connections
    ftp_control_connection_attempted: AtomicU64,
    ftp_transfer_connection_attempted: AtomicU64,
}

impl EscaperInterfaceStats {
    pub(crate) fn add_tcp_connect_attempted(&self) {
        self.tcp_connect_attempted.fetch_add(1, Ordering::Relaxed);
    }

    pub(crate) fn add_tls_connect_attempted(&self) {
        self.tls_connect_attempted.fetch_add(1, Ordering::Relaxed);
    }

    pub(crate) fn add_udp_connect_attempted(&self) {
        self.udp_connect_attempted.fetch_add(1, Ordering::Relaxed);
    }

    pub(crate) fn add_udp_relay_session_attempted(&self) {
        self.udp_relay_session_attempted
            .fetch_add(1, Ordering::Relaxed);
    }

    pub(crate) fn add_http_forward_request_attempted(&self) {
        self.http_forward_request_attempted
            .fetch_add(1, Ordering::Relaxed);
    }

    pub(crate) fn add_https_forward_request_attempted(&self) {
        self.https_forward_request_attempted
            .fetch_add(1, Ordering::Relaxed);
    }

    pub(crate) fn add_ftp_over_http_request_attempted(&self) {
        self.ftp_over_http_request_attempted
            .fetch_add(1, Ordering::Relaxed);
    }

    pub(crate) fn add_http_forward_connection_attempted(&self) {
        self.http_forward_connection_attempted
            .fetch_add(1, Ordering::Relaxed);
    }

    pub(crate) fn add_https_forward_connection_attempted(&self) {
        self.https_forward_connection_attempted
            .fetch_add(1, Ordering::Relaxed);
    }

    pub(crate) fn add_ftp_control_connection_attempted(&self) {
        self.ftp_control_connection_attempted
            .fetch_add(1, Ordering::Relaxed);
    }

    pub(crate) fn add_ftp_transfer_connection_attempted(&self) {
        self.ftp_transfer_connection_attempted
            .fetch_add(1, Ordering::Relaxed);
    }

    pub(crate) fn get_task_total(&self) -> u64 {
        self.tcp_connect_attempted.load(Ordering::Relaxed)
            + self.tls_connect_attempted.load(Ordering::Relaxed)
            + self.udp_connect_attempted.load(Ordering::Relaxed)
            + self.udp_relay_session_attempted.load(Ordering::Relaxed)
            + self.http_forward_request_attempted.load(Ordering::Relaxed)
            + self.https_forward_request_attempted.load(Ordering::Relaxed)
            + self.ftp_over_http_request_attempted.load(Ordering::Relaxed)
    }
}

#[derive(Default)]
pub(crate) struct EscaperTcpStats {
    connection_attempted: AtomicU64,
    connection_established: AtomicU64,
    pub(crate) io: TcpIoStats,
}

impl EscaperTcpStats {
    pub(crate) fn add_connection_attempted(&self) {
        self.connection_attempted.fetch_add(1, Ordering::Relaxed);
    }

    pub(crate) fn get_connection_attempted(&self) -> u64 {
        self.connection_attempted.load(Ordering::Relaxed)
    }

    pub(crate) fn add_connection_established(&self) {
        self.connection_established.fetch_add(1, Ordering::Relaxed);
    }

    pub(crate) fn get_connection_established(&self) -> u64 {
        self.connection_established.load(Ordering::Relaxed)
    }
}

#[derive(Default)]
pub(crate) struct EscaperUdpStats {
    pub(crate) io: UdpIoStats,
}

#[derive(Default)]
pub(crate) struct RouteEscaperSnapshot {
    pub(crate) request_passed: u64,
    pub(crate) request_failed: u64,
}

/// General stats for `route` type escapers
pub(crate) struct RouteEscaperStats {
    name: MetricsName,
    id: StatId,
    request_passed: AtomicU64,
    request_failed: AtomicU64,
}

impl RouteEscaperStats {
    pub(super) fn new(name: &MetricsName) -> Self {
        RouteEscaperStats {
            name: name.clone(),
            id: StatId::new(),
            request_passed: AtomicU64::new(0),
            request_failed: AtomicU64::new(0),
        }
    }

    #[inline]
    pub(crate) fn name(&self) -> &MetricsName {
        &self.name
    }

    #[inline]
    pub(crate) fn stat_id(&self) -> StatId {
        self.id
    }

    pub(crate) fn add_request_passed(&self) {
        self.request_passed.fetch_add(1, Ordering::Relaxed);
    }

    pub(crate) fn add_request_failed(&self) {
        self.request_failed.fetch_add(1, Ordering::Relaxed);
    }

    pub(crate) fn snapshot(&self) -> RouteEscaperSnapshot {
        RouteEscaperSnapshot {
            request_passed: self.request_passed.load(Ordering::Relaxed),
            request_failed: self.request_failed.load(Ordering::Relaxed),
        }
    }
}
