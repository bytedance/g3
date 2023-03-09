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
