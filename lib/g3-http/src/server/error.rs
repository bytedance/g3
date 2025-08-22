/*
 * SPDX-License-Identifier: Apache-2.0
 * Copyright 2023-2025 ByteDance and/or its affiliates.
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
    #[error("unsupported well-known uri: {0}")]
    UnsupportedRequest(String),
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
            | HttpRequestParseError::UnsupportedScheme
            | HttpRequestParseError::UnsupportedRequest(_) => Some(StatusCode::NOT_IMPLEMENTED),
            HttpRequestParseError::UnmatchedHostAndAuthority => Some(StatusCode::CONFLICT),
            HttpRequestParseError::LoopDetected => Some(StatusCode::LOOP_DETECTED),
            _ => Some(StatusCode::BAD_REQUEST),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use http::{StatusCode, Version};
    use std::io;

    #[test]
    fn status_code() {
        // None cases
        assert_eq!(HttpRequestParseError::ClientClosed.status_code(), None);
        let io_err = io::Error::other("test error");
        assert_eq!(HttpRequestParseError::IoFailed(io_err).status_code(), None);

        // Specific status codes
        assert_eq!(
            HttpRequestParseError::TooLargeHeader(1024).status_code(),
            Some(StatusCode::REQUEST_HEADER_FIELDS_TOO_LARGE)
        );
        assert_eq!(
            HttpRequestParseError::UnmatchedHostAndAuthority.status_code(),
            Some(StatusCode::CONFLICT)
        );
        assert_eq!(
            HttpRequestParseError::LoopDetected.status_code(),
            Some(StatusCode::LOOP_DETECTED)
        );

        // Not Implemented cases
        assert_eq!(
            HttpRequestParseError::UpgradeIsNotSupported.status_code(),
            Some(StatusCode::NOT_IMPLEMENTED)
        );
        assert_eq!(
            HttpRequestParseError::UnsupportedMethod("INVALID".to_string()).status_code(),
            Some(StatusCode::NOT_IMPLEMENTED)
        );
        assert_eq!(
            HttpRequestParseError::UnsupportedScheme.status_code(),
            Some(StatusCode::NOT_IMPLEMENTED)
        );
        assert_eq!(
            HttpRequestParseError::UnsupportedRequest("foo".to_string()).status_code(),
            Some(StatusCode::NOT_IMPLEMENTED)
        );

        // Bad Request cases
        let inner_err = HttpLineParseError::InvalidVersion;
        assert_eq!(
            HttpRequestParseError::InvalidMethodLine(inner_err).status_code(),
            Some(StatusCode::BAD_REQUEST)
        );
        assert_eq!(
            HttpRequestParseError::UnsupportedVersion(Version::HTTP_3).status_code(),
            Some(StatusCode::BAD_REQUEST)
        );
        assert_eq!(
            HttpRequestParseError::InvalidRequestTarget.status_code(),
            Some(StatusCode::BAD_REQUEST)
        );
        let inner_err = HttpLineParseError::InvalidHeaderName;
        assert_eq!(
            HttpRequestParseError::InvalidHeaderLine(inner_err).status_code(),
            Some(StatusCode::BAD_REQUEST)
        );
        assert_eq!(
            HttpRequestParseError::InvalidHost.status_code(),
            Some(StatusCode::BAD_REQUEST)
        );
        assert_eq!(
            HttpRequestParseError::UnsupportedAuthorization.status_code(),
            Some(StatusCode::BAD_REQUEST)
        );
        assert_eq!(
            HttpRequestParseError::MissedHost.status_code(),
            Some(StatusCode::BAD_REQUEST)
        );
        assert_eq!(
            HttpRequestParseError::InvalidChunkedTransferEncoding.status_code(),
            Some(StatusCode::BAD_REQUEST)
        );
        assert_eq!(
            HttpRequestParseError::InvalidContentLength.status_code(),
            Some(StatusCode::BAD_REQUEST)
        );
    }
}
