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
