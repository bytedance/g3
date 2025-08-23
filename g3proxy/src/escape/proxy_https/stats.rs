/*
 * SPDX-License-Identifier: Apache-2.0
 * Copyright 2024-2025 ByteDance and/or its affiliates.
 */

use std::sync::Arc;

use arc_swap::ArcSwapOption;

use g3_daemon::stat::remote::TcpConnectionTaskRemoteStats;
use g3_io_ext::{LimitedReaderStats, LimitedWriterStats};
use g3_types::metrics::{MetricTagMap, NodeName};
use g3_types::stats::{StatId, TcpIoSnapshot};

use crate::escape::{
    EscaperInterfaceStats, EscaperInternalStats, EscaperStats, EscaperTcpConnectSnapshot,
    EscaperTcpStats, EscaperTlsSnapshot, EscaperTlsStats,
};
use crate::module::http_forward::HttpForwardTaskRemoteStats;

pub(crate) struct ProxyHttpsEscaperStats {
    name: NodeName,
    id: StatId,
    extra_metrics_tags: Arc<ArcSwapOption<MetricTagMap>>,
    pub(crate) interface: EscaperInterfaceStats,
    pub(crate) tcp: EscaperTcpStats,
    pub(crate) tls: EscaperTlsStats,
}

impl ProxyHttpsEscaperStats {
    pub(crate) fn new(name: &NodeName) -> Self {
        ProxyHttpsEscaperStats {
            name: name.clone(),
            id: StatId::new_unique(),
            extra_metrics_tags: Arc::new(ArcSwapOption::new(None)),
            interface: EscaperInterfaceStats::default(),
            tcp: EscaperTcpStats::default(),
            tls: EscaperTlsStats::default(),
        }
    }

    pub(crate) fn set_extra_tags(&self, tags: Option<Arc<MetricTagMap>>) {
        self.extra_metrics_tags.store(tags);
    }
}

impl EscaperInternalStats for ProxyHttpsEscaperStats {
    #[inline]
    fn add_http_forward_request_attempted(&self) {
        self.interface.add_http_forward_request_attempted();
    }

    #[inline]
    fn add_https_forward_request_attempted(&self) {
        self.interface.add_https_forward_request_attempted();
    }
}

impl EscaperStats for ProxyHttpsEscaperStats {
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
        self.tcp.connection_attempted()
    }

    fn connection_established(&self) -> u64 {
        self.tcp.connection_established()
    }

    fn tcp_connect_snapshot(&self) -> Option<EscaperTcpConnectSnapshot> {
        Some(self.tcp.connect_snapshot())
    }

    fn tls_snapshot(&self) -> Option<EscaperTlsSnapshot> {
        Some(self.tls.snapshot())
    }

    fn tcp_io_snapshot(&self) -> Option<TcpIoSnapshot> {
        Some(self.tcp.io.snapshot())
    }
}

impl LimitedReaderStats for ProxyHttpsEscaperStats {
    fn add_read_bytes(&self, size: usize) {
        let size = size as u64;
        self.tcp.io.add_in_bytes(size);
    }
}

impl LimitedWriterStats for ProxyHttpsEscaperStats {
    fn add_write_bytes(&self, size: usize) {
        let size = size as u64;
        self.tcp.io.add_out_bytes(size);
    }
}

impl TcpConnectionTaskRemoteStats for ProxyHttpsEscaperStats {
    fn add_read_bytes(&self, size: u64) {
        self.tcp.io.add_in_bytes(size);
    }

    fn add_write_bytes(&self, size: u64) {
        self.tcp.io.add_out_bytes(size);
    }
}

impl HttpForwardTaskRemoteStats for ProxyHttpsEscaperStats {
    fn add_read_bytes(&self, size: u64) {
        self.tcp.io.add_in_bytes(size);
    }

    fn add_write_bytes(&self, size: u64) {
        self.tcp.io.add_out_bytes(size);
    }
}
