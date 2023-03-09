/*
 * Copyright 2023 ByteDance and/or its affiliates.
 *
 * Licensed under the Apache License, Version 2.0 (the "License");
 * you may not use this file except in compliance with the License.
 * You may obtain a copy of the License at
 *
 *     http://www.apache.org/licenses/LICENSE-2.0
 *
 * Unless required by applicable law or agreed to in writing, software
 * distributed under the License is distributed on an "AS IS" BASIS,
 * WITHOUT WARRANTIES OR CONDITIONS OF ANY KIND, either express or implied.
 * See the License for the specific language governing permissions and
 * limitations under the License.
 */

use std::io;

use http::{StatusCode, Version};
use thiserror::Error;

use crate::HttpLineParseError;

#[derive(Debug, Error)]
pub enum HttpRequestParseError {
    #[error("client closed")]
    ClientClosed,
    #[error("too large header, should be less than {0}")]
    TooLargeHeader(usize),
    #[error("invalid method line: {0}")]
    InvalidMethodLine(HttpLineParseError),
    #[error("unsupported method: {0}")]
    UnsupportedMethod(String),
    #[error("unsupported version: {0:?}")]
    UnsupportedVersion(Version),
    #[error("invalid request target")]
    InvalidRequestTarget,
    #[error("invalid scheme")]
    UnsupportedScheme,
    #[error("invalid header line: {0}")]
    InvalidHeaderLine(HttpLineParseError),
    #[error("invalid host header")]
    InvalidHost,
    #[error("unsupported (proxy) authorization")]
    UnsupportedAuthorization,
    #[error("missed host header")]
    MissedHost,
    #[error("unmatched host and authority")]
    UnmatchedHostAndAuthority,
    #[error("invalid chunked transfer-encoding")]
    InvalidChunkedTransferEncoding,
    #[error("invalid content length")]
    InvalidContentLength,
    #[error("upgrade is not supported")]
    UpgradeIsNotSupported,
    #[error("loop detected")]
    LoopDetected,
    #[error("io failed: {0:?}")]
    IoFailed(#[from] io::Error),
}

impl HttpRequestParseError {
    pub fn status_code(&self) -> Option<StatusCode> {
        match self {
            HttpRequestParseError::IoFailed(_) | HttpRequestParseError::ClientClosed => None,
            HttpRequestParseError::TooLargeHeader(_) => {
                Some(StatusCode::REQUEST_HEADER_FIELDS_TOO_LARGE)
            }
            HttpRequestParseError::UpgradeIsNotSupported
            | HttpRequestParseError::UnsupportedMethod(_)
            | HttpRequestParseError::UnsupportedScheme => Some(StatusCode::NOT_IMPLEMENTED),
            HttpRequestParseError::UnmatchedHostAndAuthority => Some(StatusCode::CONFLICT),
            HttpRequestParseError::LoopDetected => Some(StatusCode::LOOP_DETECTED),
            _ => Some(StatusCode::BAD_REQUEST),
        }
    }
}
