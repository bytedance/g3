/*
 * SPDX-License-Identifier: Apache-2.0
 * Copyright 2024-2025 ByteDance and/or its affiliates.
 */

use std::str;

use super::CommandLineError;

#[derive(Debug, Default, PartialEq, Eq)]
pub struct MailParam {
    reverse_path: String,
}

impl MailParam {
    #[inline]
    pub fn reverse_path(&self) -> &str {
        &self.reverse_path
    }

    pub(super) fn parse(msg: &[u8]) -> Result<Self, CommandLineError> {
        let msg = str::from_utf8(msg).map_err(CommandLineError::InvalidUtf8Command)?;

        let mut iter = msg.split_ascii_whitespace();
        let s = iter.next().ok_or(CommandLineError::InvalidCommandParam(
            "MAIL",
            "no reverse path present",
        ))?;

        let reverse_path = s
            .to_lowercase()
            .strip_prefix("from:")
            .map(|s| s.to_string())
            .ok_or(CommandLineError::InvalidCommandParam(
                "MAIL",
                "invalid reverse path prefix",
            ))?;
        if !super::path::is_valid(&reverse_path) {
            return Err(CommandLineError::InvalidCommandParam(
                "MAIL",
                "invalid reverse path prefix",
            ));
        }

        Ok(MailParam { reverse_path })
    }
}
