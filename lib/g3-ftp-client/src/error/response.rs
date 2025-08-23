/*
 * SPDX-License-Identifier: Apache-2.0
 * Copyright 2023-2025 ByteDance and/or its affiliates.
 */

use std::io;

use thiserror::Error;

#[derive(Debug, Error)]
pub enum FtpRawResponseError {
    #[error("read failed: {0:?}")]
    ReadFailed(io::Error),
    #[error("connection closed")]
    ConnectionClosed,
    #[error("line too long")]
    LineTooLong,
    #[error("invalid line format")]
    InvalidLineFormat,
    #[error("invalid reply code {0}")]
    InvalidReplyCode(u16),
    #[error("line is not utf8")]
    LineIsNotUtf8,
    #[error("too many lines")]
    TooManyLines,
    #[error("read response for stage '{0}' timed out")]
    ReadResponseTimedOut(&'static str),
}
