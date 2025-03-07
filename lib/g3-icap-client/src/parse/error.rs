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
