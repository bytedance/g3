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
