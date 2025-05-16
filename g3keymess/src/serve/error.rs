/*
 * SPDX-License-Identifier: Apache-2.0
 * Copyright 2023-2025 ByteDance and/or its affiliates.
 */

use std::io;

use thiserror::Error;

use crate::protocol::KeylessRequestError;

#[derive(Debug, Error)]
pub(crate) enum ServerTaskError {
    #[error("no error")]
    NoError,
    #[error("connection closed early")]
    ConnectionClosedEarly,
    #[error("write failed: {0:?}")]
    WriteFailed(io::Error),
    #[error("read failed: {0:?}")]
    ReadFailed(io::Error),
    #[error("invalid request: {0}")]
    InvalidRequest(KeylessRequestError),
    #[error("read request timeout")]
    ReadTimeout,
    #[error("server force quit")]
    ServerForceQuit,
}

impl ServerTaskError {
    pub(crate) fn ignore_log(&self) -> bool {
        matches!(self, ServerTaskError::ConnectionClosedEarly)
    }
}

impl From<KeylessRequestError> for ServerTaskError {
    fn from(value: KeylessRequestError) -> Self {
        match value {
            KeylessRequestError::ClosedEarly => ServerTaskError::ConnectionClosedEarly,
            KeylessRequestError::ReadFailed(e) => ServerTaskError::ReadFailed(e),
            _ => ServerTaskError::InvalidRequest(value),
        }
    }
}
