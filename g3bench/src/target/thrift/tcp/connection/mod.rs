/*
 * SPDX-License-Identifier: Apache-2.0
 * Copyright 2025 ByteDance and/or its affiliates.
 */

use std::io;

use thiserror::Error;

use crate::target::thrift::protocol::{ThriftResponseMessage, ThriftResponseMessageParseError};

mod simplex;
pub(super) use simplex::SimplexTransfer;

mod multiplex;
pub(super) use multiplex::MultiplexTransfer;

#[derive(Debug, Error)]
pub(super) enum ThriftTcpResponseError {
    #[error("invalid request: {0}")]
    InvalidRequest(anyhow::Error),
    #[error("write failed: {0}")]
    WriteFailed(io::Error),
    #[error("read failed: {0}")]
    ReadFailed(io::Error),
    #[error("no enough data read")]
    NoEnoughDataRead,
    #[error("invalid message: {0}")]
    InvalidMessage(#[from] ThriftResponseMessageParseError),
}

pub(crate) struct ThriftTcpResponse {
    pub(super) seq_id: i32,
    pub(super) message: ThriftResponseMessage,
}
