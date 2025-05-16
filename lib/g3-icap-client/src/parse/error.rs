/*
 * SPDX-License-Identifier: Apache-2.0
 * Copyright 2023-2025 ByteDance and/or its affiliates.
 */

use std::str::Utf8Error;

use thiserror::Error;

#[derive(Debug, Error)]
pub enum IcapLineParseError {
    #[error("not long enough")]
    NotLongEnough,
    #[error("no delimiter '{0}' found")]
    NoDelimiterFound(char),
    #[error("missing header name")]
    MissingHeaderName,
    #[error("invalid utf8 encoding: {0}")]
    InvalidUtf8Encoding(#[from] Utf8Error),
    #[error("invalid icap version")]
    InvalidIcapVersion,
    #[error("invalid status code")]
    InvalidStatusCode,
    #[error("invalid header name")]
    InvalidHeaderName,
    #[error("invalid header value")]
    InvalidHeaderValue,
}
