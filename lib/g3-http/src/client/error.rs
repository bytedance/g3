/*
 * SPDX-License-Identifier: Apache-2.0
 * Copyright 2023-2025 ByteDance and/or its affiliates.
 */

use std::io;

use http::Version;
use thiserror::Error;

use g3_types::net::HttpUpgradeTokenParseError;

use crate::HttpLineParseError;

#[derive(Debug, Error)]
pub enum HttpResponseParseError {
    #[error("remote closed")]
    RemoteClosed,
    #[error("too large header, should be less than {0}")]
    TooLargeHeader(usize),
    #[error("invalid version {0:?}")]
    InvalidVersion(Version),
    #[error("invalid status line: {0}")]
    InvalidStatusLine(HttpLineParseError),
    #[error("invalid header line: {0}")]
    InvalidHeaderLine(HttpLineParseError),
    #[error("invalid chunked transfer-encoding")]
    InvalidChunkedTransferEncoding,
    #[error("invalid content length")]
    InvalidContentLength,
    #[error("invalid upgrade protocol: {0}")]
    InvalidUpgradeProtocol(#[from] HttpUpgradeTokenParseError),
    #[error("io failed: {0:?}")]
    IoFailed(#[from] io::Error),
}
