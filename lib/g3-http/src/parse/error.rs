/*
 * SPDX-License-Identifier: Apache-2.0
 * Copyright 2023-2025 ByteDance and/or its affiliates.
 */

use std::str::Utf8Error;

use thiserror::Error;

#[derive(Debug, Error)]
pub enum HttpLineParseError {
    #[error("not long enough")]
    NotLongEnough,
    #[error("too long line (> {0})")]
    LineTooLong(usize),
    #[error("invalid utf-8 encoding: {0}")]
    InvalidUtf8Encoding(#[from] Utf8Error),
    #[error("no delimiter '{0}' found")]
    NoDelimiterFound(char),
    #[error("invalid header name")]
    InvalidHeaderName,
    #[error("invalid header value")]
    InvalidHeaderValue,
    #[error("invalid version")]
    InvalidVersion,
    #[error("invalid status code")]
    InvalidStatusCode,
    #[error("invalid chunk size")]
    InvalidChunkSize,
}
