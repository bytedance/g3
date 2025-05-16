/*
 * SPDX-License-Identifier: Apache-2.0
 * Copyright 2024-2025 ByteDance and/or its affiliates.
 */

use tokio::sync::oneshot;
use tokio::time::Instant;

use crate::module::keyless::{KeylessRequest, KeylessResponse};

mod stats;
pub(crate) use stats::{
    KeylessBackendAliveChannelGuard, KeylessBackendStats, KeylessUpstreamDurationRecorder,
    KeylessUpstreamDurationStats,
};

mod pool;
pub(crate) use pool::{
    KeylessConnectionPool, KeylessConnectionPoolHandle, KeylessUpstreamConnect,
    KeylessUpstreamConnection,
};

mod multiplex;
pub(crate) use multiplex::{MultiplexedUpstreamConnection, MultiplexedUpstreamConnectionConfig};

pub(crate) struct KeylessForwardRequest {
    created: Instant,
    req: KeylessRequest,
    rsp_sender: oneshot::Sender<KeylessResponse>,
}

impl KeylessForwardRequest {
    pub(crate) fn new(req: KeylessRequest, rsp_sender: oneshot::Sender<KeylessResponse>) -> Self {
        KeylessForwardRequest {
            created: Instant::now(),
            req,
            rsp_sender,
        }
    }
}
