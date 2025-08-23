/*
 * SPDX-License-Identifier: Apache-2.0
 * Copyright 2023-2025 ByteDance and/or its affiliates.
 */

mod listen;
pub use listen::emit_listen_stats;

#[cfg(feature = "event-log")]
mod log;
#[cfg(feature = "event-log")]
pub(crate) use log::{LoggerMetricExt, emit_log_drop_stats, emit_log_io_stats};

mod server;
pub use server::{ServerMetricExt, TAG_KEY_ONLINE, TAG_KEY_SERVER};

pub mod helper;

pub const TAG_KEY_DAEMON_GROUP: &str = "daemon_group";

pub const TAG_KEY_STAT_ID: &str = "stat_id";
pub const TAG_KEY_TRANSPORT: &str = "transport";
pub const TAG_KEY_CONNECTION: &str = "connection";
pub const TAG_KEY_REQUEST: &str = "request";
pub const TAG_KEY_QUANTILE: &str = "quantile";

pub const TRANSPORT_TYPE_TCP: &str = "tcp";
pub const TRANSPORT_TYPE_UDP: &str = "udp";

#[derive(Copy, Clone)]
pub enum MetricTransportType {
    Tcp,
    Udp,
}

impl MetricTransportType {
    pub const fn as_str(&self) -> &'static str {
        match self {
            MetricTransportType::Tcp => TRANSPORT_TYPE_TCP,
            MetricTransportType::Udp => TRANSPORT_TYPE_UDP,
        }
    }
}

impl AsRef<str> for MetricTransportType {
    fn as_ref(&self) -> &str {
        self.as_str()
    }
}
