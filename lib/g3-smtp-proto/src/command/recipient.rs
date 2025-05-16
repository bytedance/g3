/*
 * SPDX-License-Identifier: Apache-2.0
 * Copyright 2024-2025 ByteDance and/or its affiliates.
 */

use std::str;

use super::CommandLineError;

#[derive(Debug, Default, PartialEq, Eq)]
pub struct RecipientParam {
    forward_path: String,
}

impl RecipientParam {
    #[inline]
    pub fn forward_path(&self) -> &str {
        &self.forward_path
    }

    pub(super) fn parse(msg: &[u8]) -> Result<Self, CommandLineError> {
        let msg = str::from_utf8(msg).map_err(CommandLineError::InvalidUtf8Command)?;

        let mut iter = msg.split_ascii_whitespace();
        let s = iter.next().ok_or(CommandLineError::InvalidCommandParam(
            "RCPT",
            "no forward path present",
        ))?;

        let forward_path = s
            .to_lowercase()
            .strip_prefix("to:")
            .map(|s| s.to_string())
            .ok_or(CommandLineError::InvalidCommandParam(
                "RCPT",
                "invalid forward path prefix",
            ))?;
        if !super::path::is_valid(&forward_path) {
            return Err(CommandLineError::InvalidCommandParam(
                "RCPT",
                "invalid forward path prefix",
            ));
        }

        Ok(RecipientParam { forward_path })
    }
}
