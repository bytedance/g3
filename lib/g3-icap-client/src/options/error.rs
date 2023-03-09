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
