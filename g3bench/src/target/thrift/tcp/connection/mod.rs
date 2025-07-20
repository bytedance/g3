/*
 * SPDX-License-Identifier: Apache-2.0
 * Copyright 2025 ByteDance and/or its affiliates.
 */

use std::io;

use thiserror::Error;

mod simplex;
pub(super) use simplex::SimplexTransfer;

mod multiplex;
pub(super) use multiplex::MultiplexTransfer;

pub(super) struct ThriftTcpRequest {
    pub(super) seq_id: i32,
    pub(super) buf: Vec<u8>,
}

#[derive(Debug, Error)]
pub(super) enum ThriftTcpResponseLocalError {
    #[error("invalid request: {0}")]
    InvalidRequest(anyhow::Error),
    #[error("write failed: {0}")]
    WriteFailed(io::Error),
    #[error("read failed: {0}")]
    ReadFailed(io::Error),
}

#[derive(Debug, Error)]
pub(super) enum ThriftTcpResponseError {
    #[error("local error: {0}")]
    Local(#[from] ThriftTcpResponseLocalError),
}

pub(crate) struct ThriftTcpResponse {
    pub(super) seq_id: i32,
}
