/*
 * SPDX-License-Identifier: Apache-2.0
 * Copyright 2024-2025 ByteDance and/or its affiliates.
 */

use tokio::io::{AsyncRead, AsyncWrite};

mod stats;
pub(crate) use stats::{
    StreamAcceptTaskCltWrapperStats, StreamBackendDurationRecorder, StreamBackendDurationStats,
    StreamBackendStats, StreamRelayTaskCltWrapperStats, StreamServerAliveTaskGuard,
    StreamServerStats,
};

mod error;
pub(crate) use error::StreamConnectError;

pub(crate) type ConnectedStream = (
    Box<dyn AsyncRead + Unpin + Send + Sync>,
    Box<dyn AsyncWrite + Unpin + Send + Sync>,
);
pub(crate) type StreamConnectResult = Result<ConnectedStream, StreamConnectError>;

mod transit;
pub(crate) use transit::StreamTransitTask;
