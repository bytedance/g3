/*
 * SPDX-License-Identifier: Apache-2.0
 * Copyright 2023-2025 ByteDance and/or its affiliates.
 */

use std::io;

use thiserror::Error;

use crate::HttpLineParseError;

#[derive(Debug, Error)]
pub enum HttpConnectResponseError {
    #[error("too large header, should be less than {0}")]
    TooLargeHeader(usize),
    #[error("invalid status line: {0}")]
    InvalidStatusLine(HttpLineParseError),
    #[error("invalid header line: {0}")]
    InvalidHeaderLine(HttpLineParseError),
    #[error("invalid chunked transfer-encoding")]
    InvalidChunkedTransferEncoding,
    #[error("invalid content length")]
    InvalidContentLength,
}

#[derive(Debug, Error)]
pub enum HttpConnectError {
    #[error("remote closed")]
    RemoteClosed,
    #[error("read failed: {0:?}")]
    ReadFailed(io::Error),
    #[error("write failed: {0:?}")]
    WriteFailed(io::Error),
    #[error("invalid response: {0}")]
    InvalidResponse(#[from] HttpConnectResponseError),
    #[error("unexpected status code {0} {1}")]
    UnexpectedStatusCode(u16, String),
    #[error("peer timeout with status code {0}")]
    PeerTimeout(u16),
}
