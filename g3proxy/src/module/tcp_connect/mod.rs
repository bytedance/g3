/*
 * SPDX-License-Identifier: Apache-2.0
 * Copyright 2023-2025 ByteDance and/or its affiliates.
 */

use tokio::io::{AsyncRead, AsyncWrite};

mod error;
mod stats;
mod task;

pub(crate) use error::TcpConnectError;
pub(crate) use stats::TcpConnectRemoteWrapperStats;
pub(crate) use task::{TcpConnectTaskConf, TcpConnectTaskNotes, TlsConnectTaskConf};

pub(crate) type TcpConnection = (
    Box<dyn AsyncRead + Unpin + Send + Sync>,
    Box<dyn AsyncWrite + Unpin + Send + Sync>,
);
pub(crate) type TcpConnectResult = Result<TcpConnection, TcpConnectError>;
