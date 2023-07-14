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

use crate::protocol::KeylessRequestError;

#[derive(Debug, Error)]
pub(crate) enum ServerTaskError {
    #[error("no error")]
    NoError,
    #[error("write failed: {0:?}")]
    WriteFailed(io::Error),
    #[error("read failed: {0:?}")]
    ReadFailed(io::Error),
    #[error("invalid request: {0}")]
    InvalidRequest(KeylessRequestError),
    #[error("read request timeout")]
    ReadTimeout,
    #[error("server force quit")]
    ServerForceQuit,
}

impl From<KeylessRequestError> for ServerTaskError {
    fn from(value: KeylessRequestError) -> Self {
        match value {
            KeylessRequestError::ReadFailed(e) => ServerTaskError::ReadFailed(e),
            _ => ServerTaskError::InvalidRequest(value),
        }
    }
}
