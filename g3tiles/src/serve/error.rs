/*
 * SPDX-License-Identifier: Apache-2.0
 * Copyright 2023-2025 ByteDance and/or its affiliates.
 */

use std::io;
use std::time::Duration;

use thiserror::Error;

use g3_types::net::ConnectError;

use crate::module::stream::StreamConnectError;

#[derive(Error, Debug)]
pub(crate) enum ServerTaskError {
    #[error("internal server error: {0}")]
    InternalServerError(&'static str),
    #[error("tcp read from client: {0:?}")]
    ClientTcpReadFailed(io::Error),
    #[error("tcp write to client: {0:?}")]
    ClientTcpWriteFailed(io::Error),
    #[error("invalid client protocol: {0}")]
    InvalidClientProtocol(&'static str),
    #[error("upstream not resolved")]
    UpstreamNotResolved,
    #[error("upstream not connected: {0}")]
    UpstreamNotConnected(ConnectError),
    #[error("read from upstream: {0:?}")]
    UpstreamReadFailed(io::Error),
    #[error("write to upstream: {0:?}")]
    UpstreamWriteFailed(io::Error),
    #[error("closed by client")]
    ClosedByClient,
    #[error("canceled as server quit")]
    CanceledAsServerQuit,
    #[error("idle after {0:?} x {1}")]
    Idle(Duration, usize),
    #[allow(unused)]
    #[error("finished")]
    Finished, // this isn't an error, for log only
    #[error("unclassified error: {0:?}")]
    UnclassifiedError(#[from] anyhow::Error),
}

impl ServerTaskError {
    pub(crate) fn brief(&self) -> &'static str {
        match self {
            ServerTaskError::InternalServerError(_) => "InternalServerError",
            ServerTaskError::ClientTcpReadFailed(_) => "ClientTcpReadFailed",
            ServerTaskError::ClientTcpWriteFailed(_) => "ClientTcpWriteFailed",
            ServerTaskError::InvalidClientProtocol(_) => "InvalidClientProtocol",
            ServerTaskError::UpstreamNotResolved => "UpstreamNotResolved",
            ServerTaskError::UpstreamNotConnected(_) => "UpstreamNotConnected",
            ServerTaskError::UpstreamReadFailed(_) => "UpstreamReadFailed",
            ServerTaskError::UpstreamWriteFailed(_) => "UpstreamWriteFailed",
            ServerTaskError::ClosedByClient => "ClosedByClient",
            ServerTaskError::CanceledAsServerQuit => "CanceledAsServerQuit",
            ServerTaskError::Idle(_, _) => "Idle",
            ServerTaskError::Finished => "Finished",
            ServerTaskError::UnclassifiedError(_) => "UnclassifiedError",
        }
    }
}

pub(crate) type ServerTaskResult<T> = Result<T, ServerTaskError>;

impl From<StreamConnectError> for ServerTaskError {
    fn from(value: StreamConnectError) -> Self {
        match value {
            StreamConnectError::UpstreamNotResolved => ServerTaskError::UpstreamNotResolved,
            StreamConnectError::SetupSocketFailed(_) => ServerTaskError::InternalServerError(
                "failed to setup local socket for remote connection",
            ),
            StreamConnectError::ConnectFailed(e) => ServerTaskError::UpstreamNotConnected(e),
        }
    }
}
