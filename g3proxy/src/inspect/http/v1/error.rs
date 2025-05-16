/*
 * SPDX-License-Identifier: Apache-2.0
 * Copyright 2023-2025 ByteDance and/or its affiliates.
 */

use std::io;

use thiserror::Error;

use g3_http::server::HttpRequestParseError;
use g3_types::net::HttpUpgradeToken;

#[derive(Debug, Error)]
pub(crate) enum H1InterceptionError {
    #[error("closed by client")]
    ClosedByClient,
    #[error("client read: {0:?}")]
    ClientReadFailed(io::Error),
    #[error("client application timeout: {0}")]
    ClientAppTimeout(&'static str),
    #[error("invalid request: {0}")]
    InvalidRequestHeader(HttpRequestParseError),
    #[error("closed by upstream")]
    ClosedByUpstream,
    #[error("unexpected data from upstream")]
    UnexpectedUpstreamData,
    #[error("upstream closed with error: {0:?}")]
    UpstreamClosedWithError(io::Error),
    #[error("invalid upgrade protocol: {0}")]
    InvalidUpgradeProtocol(HttpUpgradeToken),
}

impl From<HttpRequestParseError> for H1InterceptionError {
    fn from(e: HttpRequestParseError) -> Self {
        match e {
            HttpRequestParseError::ClientClosed => H1InterceptionError::ClosedByClient,
            HttpRequestParseError::IoFailed(e) => H1InterceptionError::ClientReadFailed(e),
            _ => H1InterceptionError::InvalidRequestHeader(e),
        }
    }
}
