/*
 * SPDX-License-Identifier: Apache-2.0
 * Copyright 2023-2025 ByteDance and/or its affiliates.
 */

use std::sync::Arc;
use std::sync::atomic::{AtomicU64, Ordering};

use arc_swap::ArcSwapOption;

use g3_types::metrics::{MetricTagMap, NodeName};
use g3_types::stats::{StatId, TcpIoSnapshot, TcpIoStats, UdpIoSnapshot, UdpIoStats};

pub(crate) trait EscaperInternalStats {
    fn add_http_forward_request_attempted(&self);
    fn add_https_forward_request_attempted(&self);
}

pub(crate) trait EscaperStats: EscaperInternalStats {
    fn name(&self) -> &NodeName;
    fn stat_id(&self) -> StatId;
    fn load_extra_tags(&self) -> Option<Arc<MetricTagMap>>;
    fn share_extra_tags(&self) -> &Arc<ArcSwapOption<MetricTagMap>>;

    /// count for tasks
    fn get_task_total(&self) -> u64;

    /// count for attempted established connections
    fn connection_attempted(&self) -> u64;
    fn connection_established(&self) -> u64;

    fn tcp_connect_snapshot(&self) -> Option<EscaperTcpConnectSnapshot> {
        None
    }

    fn tls_snapshot(&self) -> Option<EscaperTlsSnapshot> {
        None
    }

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
pub(crate) struct EscaperTcpConnectSnapshot {
    pub(crate) attempt: u64,
    pub(crate) establish: u64,
    pub(crate) success: u64,
    pub(crate) error: u64,
    pub(crate) timeout: u64,
}

#[derive(Default)]
pub(super) struct EscaperTcpConnectStats {
    attempted: AtomicU64,
    established: AtomicU64,
    success: AtomicU64,
    error: AtomicU64,
    timeout: AtomicU64,
}

impl EscaperTcpConnectStats {
    pub(super) fn add_attempted(&self) {
        self.attempted.fetch_add(1, Ordering::Relaxed);
    }

    pub(super) fn add_established(&self) {
        self.established.fetch_add(1, Ordering::Relaxed);
    }

    pub(super) fn add_success(&self) {
        self.success.fetch_add(1, Ordering::Relaxed);
    }

    pub(super) fn add_error(&self) {
        self.error.fetch_add(1, Ordering::Relaxed);
    }

    pub(super) fn add_timeout(&self) {
        self.error.fetch_add(1, Ordering::Relaxed);
    }

    fn snapshot(&self) -> EscaperTcpConnectSnapshot {
        EscaperTcpConnectSnapshot {
            attempt: self.attempted.load(Ordering::Relaxed),
            establish: self.established.load(Ordering::Relaxed),
            success: self.success.load(Ordering::Relaxed),
            error: self.error.load(Ordering::Relaxed),
            timeout: self.timeout.load(Ordering::Relaxed),
        }
    }
}

#[derive(Default)]
pub(crate) struct EscaperTcpStats {
    pub(super) connect: EscaperTcpConnectStats,
    pub(crate) io: TcpIoStats,
}

impl EscaperTcpStats {
    pub(crate) fn connection_attempted(&self) -> u64 {
        self.connect.attempted.load(Ordering::Relaxed)
    }

    pub(crate) fn connection_established(&self) -> u64 {
        self.connect.established.load(Ordering::Relaxed)
    }

    pub(crate) fn connect_snapshot(&self) -> EscaperTcpConnectSnapshot {
        self.connect.snapshot()
    }
}

#[derive(Default)]
pub(crate) struct EscaperUdpStats {
    pub(crate) io: UdpIoStats,
}

#[derive(Default)]
pub(crate) struct EscaperTlsSnapshot {
    pub(crate) handshake_success: u64,
    pub(crate) handshake_error: u64,
    pub(crate) handshake_timeout: u64,
    pub(crate) peer_orderly_closure: u64,
    pub(crate) peer_abortive_closure: u64,
}

#[derive(Default)]
pub(crate) struct EscaperTlsStats {
    handshake_success: AtomicU64,
    handshake_error: AtomicU64,
    handshake_timeout: AtomicU64,
    peer_orderly_closure: AtomicU64,
    peer_abortive_closure: AtomicU64,
}

impl EscaperTlsStats {
    pub(super) fn add_handshake_success(&self) {
        self.handshake_success.fetch_add(1, Ordering::Relaxed);
    }

    pub(super) fn add_handshake_error(&self) {
        self.handshake_error.fetch_add(1, Ordering::Relaxed);
    }

    pub(super) fn add_handshake_timeout(&self) {
        self.handshake_timeout.fetch_add(1, Ordering::Relaxed);
    }

    pub(super) fn add_peer_orderly_closure(&self) {
        self.peer_abortive_closure.fetch_add(1, Ordering::Relaxed);
    }

    pub(super) fn add_peer_abortive_closure(&self) {
        self.peer_abortive_closure.fetch_add(1, Ordering::Relaxed);
    }

    pub(super) fn snapshot(&self) -> EscaperTlsSnapshot {
        EscaperTlsSnapshot {
            handshake_success: self.handshake_success.load(Ordering::Relaxed),
            handshake_error: self.handshake_error.load(Ordering::Relaxed),
            handshake_timeout: self.handshake_timeout.load(Ordering::Relaxed),
            peer_orderly_closure: self.peer_orderly_closure.load(Ordering::Relaxed),
            peer_abortive_closure: self.peer_abortive_closure.load(Ordering::Relaxed),
        }
    }
}

#[derive(Default)]
pub(crate) struct RouteEscaperSnapshot {
    pub(crate) request_passed: u64,
    pub(crate) request_failed: u64,
}

/// General stats for `route` type escapers
pub(crate) struct RouteEscaperStats {
    name: NodeName,
    id: StatId,
    request_passed: AtomicU64,
    request_failed: AtomicU64,
}

impl RouteEscaperStats {
    pub(super) fn new(name: &NodeName) -> Self {
        RouteEscaperStats {
            name: name.clone(),
            id: StatId::new_unique(),
            request_passed: AtomicU64::new(0),
            request_failed: AtomicU64::new(0),
        }
    }

    #[inline]
    pub(crate) fn name(&self) -> &NodeName {
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
