/*
 * SPDX-License-Identifier: Apache-2.0
 * Copyright 2023-2025 ByteDance and/or its affiliates.
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
