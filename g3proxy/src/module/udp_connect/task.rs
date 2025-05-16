/*
 * SPDX-License-Identifier: Apache-2.0
 * Copyright 2023-2025 ByteDance and/or its affiliates.
 */

use std::net::SocketAddr;

use chrono::{DateTime, Utc};

use g3_socket::BindAddr;
use g3_types::metrics::NodeName;
use g3_types::net::{SocketBufferConfig, UpstreamAddr};

pub(crate) struct UdpConnectTaskConf<'a> {
    pub(crate) upstream: &'a UpstreamAddr,
    pub(crate) sock_buf: SocketBufferConfig,
}

#[derive(Clone, Debug, Default)]
pub(crate) struct UdpConnectTaskNotes {
    pub(crate) escaper: NodeName,
    pub(crate) bind: BindAddr,
    pub(crate) next: Option<SocketAddr>,
    pub(crate) local: Option<SocketAddr>,
    pub(crate) expire: Option<DateTime<Utc>>,
}
