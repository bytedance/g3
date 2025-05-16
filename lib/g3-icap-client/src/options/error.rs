/*
 * SPDX-License-Identifier: Apache-2.0
 * Copyright 2023-2025 ByteDance and/or its affiliates.
 */

use std::io;

use thiserror::Error;

use crate::parse::IcapLineParseError;

#[derive(Debug, Error)]
pub(crate) enum IcapOptionsParseError {
    #[error("remote closed")]
    RemoteClosed,
    #[error("too large header, should be less than {0}")]
    TooLargeHeader(usize),
    #[error("invalid status line: {0}")]
    InvalidStatusLine(IcapLineParseError),
    #[error("request failed: {0} {1}")]
    RequestFailed(u16, String),
    #[error("invalid header line: {0}")]
    InvalidHeaderLine(IcapLineParseError),
    #[error("method not match")]
    MethodNotMatch,
    #[error("no ISTag set")]
    NoServiceTagSet,
    #[error("unsupported body")]
    UnsupportedBody(String),
    #[error("invalid value for header {0}")]
    InvalidHeaderValue(&'static str),
    #[error("io failed: {0:?}")]
    IoFailed(#[from] io::Error),
}
