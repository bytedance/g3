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
pub enum CommandError {
    #[error("cli error ({0:?})")]
    Cli(#[from] anyhow::Error),
    #[error("rpc error ({0:?})")]
    Rpc(#[from] capnp::Error),
    #[error("api error (code: {code:?}, reason: {reason:?})")]
    Api { code: i32, reason: String },
    #[error("utf8 decoding error for field {field:?}: {reason:?}")]
    Utf8 {
        field: &'static str,
        reason: Utf8Error,
    },
}

impl CommandError {
    pub fn api_error(code: i32, reason_reader: capnp::text::Reader<'_>) -> Self {
        match reason_reader.to_str() {
            Ok(reason) => CommandError::Api {
                code,
                reason: reason.to_string(),
            },
            Err(e) => CommandError::Utf8 {
                field: "reason",
                reason: e,
            },
        }
    }
}

pub type CommandResult<T> = Result<T, CommandError>;
