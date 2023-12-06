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

use g3_daemon::stat::remote::TcpConnectionTaskRemoteStats;
use g3_io_ext::{LimitedReaderStats, LimitedWriterStats};
use g3_types::metrics::{MetricsName, StaticMetricsTags};
use g3_types::stats::{StatId, TcpIoSnapshot, UdpIoSnapshot};

use crate::escape::{
    EscaperForbiddenSnapshot, EscaperForbiddenStats, EscaperInterfaceStats, EscaperInternalStats,
    EscaperStats, EscaperTcpStats, EscaperUdpStats,
};
use crate::module::ftp_over_http::{FtpTaskRemoteControlStats, FtpTaskRemoteTransferStats};
use crate::module::http_forward::HttpForwardTaskRemoteStats;
use crate::module::udp_connect::UdpConnectTaskRemoteStats;
use crate::module::udp_relay::UdpRelayTaskRemoteStats;

pub(crate) struct DirectFixedEscaperStats {
    name: MetricsName,
    id: StatId,
    extra_metrics_tags: Arc<ArcSwapOption<StaticMetricsTags>>,
    pub(crate) forbidden: EscaperForbiddenStats,
    pub(crate) interface: EscaperInterfaceStats,
    pub(crate) udp: EscaperUdpStats,
    pub(crate) tcp: EscaperTcpStats,
}

impl DirectFixedEscaperStats {
    pub(crate) fn new(name: &MetricsName) -> Self {
        DirectFixedEscaperStats {
            name: name.clone(),
            id: StatId::new(),
            extra_metrics_tags: Arc::new(ArcSwapOption::new(None)),
            forbidden: Default::default(),
            interface: Default::default(),
            udp: Default::default(),
            tcp: Default::default(),
        }
    }

    pub(crate) fn set_extra_tags(&self, tags: Option<Arc<StaticMetricsTags>>) {
        self.extra_metrics_tags.store(tags);
    }
}

impl EscaperInternalStats for DirectFixedEscaperStats {
    #[inline]
    fn add_http_forward_request_attempted(&self) {
        self.interface.add_http_forward_request_attempted();
    }

    #[inline]
    fn add_https_forward_request_attempted(&self) {
        self.interface.add_https_forward_request_attempted();
    }
}

impl EscaperStats for DirectFixedEscaperStats {
    fn name(&self) -> &MetricsName {
        &self.name
    }

    fn stat_id(&self) -> StatId {
        self.id
    }

    fn load_extra_tags(&self) -> Option<Arc<StaticMetricsTags>> {
        self.extra_metrics_tags.load_full()
    }

    fn share_extra_tags(&self) -> &Arc<ArcSwapOption<StaticMetricsTags>> {
        &self.extra_metrics_tags
    }

    fn get_task_total(&self) -> u64 {
        self.interface.get_task_total()
    }

    fn get_conn_attempted(&self) -> u64 {
        self.tcp.get_connection_attempted()
    }

    fn get_conn_established(&self) -> u64 {
        self.tcp.get_connection_established()
    }

    #[inline]
    fn tcp_io_snapshot(&self) -> Option<TcpIoSnapshot> {
        Some(self.tcp.io.snapshot())
    }

    #[inline]
    fn udp_io_snapshot(&self) -> Option<UdpIoSnapshot> {
        Some(self.udp.io.snapshot())
    }

    #[inline]
    fn forbidden_snapshot(&self) -> Option<EscaperForbiddenSnapshot> {
        Some(self.forbidden.snapshot())
    }
}

impl LimitedReaderStats for DirectFixedEscaperStats {
    fn add_read_bytes(&self, size: usize) {
        let size = size as u64;
        self.tcp.io.add_in_bytes(size);
    }
}

impl LimitedWriterStats for DirectFixedEscaperStats {
    fn add_write_bytes(&self, size: usize) {
        let size = size as u64;
        self.tcp.io.add_out_bytes(size);
    }
}

impl TcpConnectionTaskRemoteStats for DirectFixedEscaperStats {
    fn add_read_bytes(&self, size: u64) {
        self.tcp.io.add_in_bytes(size);
    }

    fn add_write_bytes(&self, size: u64) {
        self.tcp.io.add_out_bytes(size);
    }
}

impl HttpForwardTaskRemoteStats for DirectFixedEscaperStats {
    fn add_read_bytes(&self, size: u64) {
        self.tcp.io.add_in_bytes(size);
    }

    fn add_write_bytes(&self, size: u64) {
        self.tcp.io.add_out_bytes(size);
    }
}

impl FtpTaskRemoteControlStats for DirectFixedEscaperStats {
    fn add_read_bytes(&self, size: u64) {
        self.tcp.io.add_in_bytes(size);
    }

    fn add_write_bytes(&self, size: u64) {
        self.tcp.io.add_out_bytes(size);
    }
}

impl FtpTaskRemoteTransferStats for DirectFixedEscaperStats {
    fn add_read_bytes(&self, size: u64) {
        self.tcp.io.add_in_bytes(size);
    }

    fn add_write_bytes(&self, size: u64) {
        self.tcp.io.add_out_bytes(size);
    }
}

impl UdpRelayTaskRemoteStats for DirectFixedEscaperStats {
    fn add_recv_bytes(&self, size: u64) {
        self.udp.io.add_in_bytes(size);
    }

    fn add_recv_packets(&self, n: usize) {
        self.udp.io.add_in_packets(n);
    }

    fn add_send_bytes(&self, size: u64) {
        self.udp.io.add_out_bytes(size);
    }

    fn add_send_packets(&self, n: usize) {
        self.udp.io.add_out_packets(n);
    }
}

impl UdpConnectTaskRemoteStats for DirectFixedEscaperStats {
    fn add_recv_bytes(&self, size: u64) {
        self.udp.io.add_in_bytes(size);
    }

    fn add_recv_packets(&self, n: usize) {
        self.udp.io.add_in_packets(n);
    }

    fn add_send_bytes(&self, size: u64) {
        self.udp.io.add_out_bytes(size);
    }

    fn add_send_packets(&self, n: usize) {
        self.udp.io.add_out_packets(n);
    }
}
