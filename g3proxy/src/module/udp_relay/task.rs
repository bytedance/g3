/*
 * SPDX-License-Identifier: Apache-2.0
 * Copyright 2023-2025 ByteDance and/or its affiliates.
 */

use chrono::{DateTime, Utc};

use g3_types::metrics::NodeName;
use g3_types::net::{SocketBufferConfig, UpstreamAddr};

pub(crate) struct UdpRelayTaskConf<'a> {
    pub(crate) initial_peer: &'a UpstreamAddr,
    pub(crate) sock_buf: SocketBufferConfig,
}

#[derive(Clone, Debug, Default)]
pub(crate) struct UdpRelayTaskNotes {
    pub(crate) escaper: NodeName,
    pub(crate) expire: Option<DateTime<Utc>>,
}
