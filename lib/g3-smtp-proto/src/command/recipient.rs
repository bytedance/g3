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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn recipient_param_parse() {
        // Valid
        let result = RecipientParam::parse(b"TO:<user@example.com>").unwrap();
        assert_eq!(result.forward_path(), "<user@example.com>");

        let result = RecipientParam::parse(b"To:<postmaster@example.com>").unwrap();
        assert_eq!(result.forward_path(), "<postmaster@example.com>");

        let result = RecipientParam::parse(b"to:<>").unwrap();
        assert_eq!(result.forward_path(), "<>");

        // Invalid UTF-8 sequences
        let result = RecipientParam::parse(b"TO:\xff\xfe<user@example.com>").unwrap_err();
        assert!(matches!(result, CommandLineError::InvalidUtf8Command(_)));

        let result = RecipientParam::parse(b"TO:<\xff\xfe>").unwrap_err();
        assert!(matches!(result, CommandLineError::InvalidUtf8Command(_)));

        // No forward path present
        let result = RecipientParam::parse(b"").unwrap_err();
        assert!(matches!(
            result,
            CommandLineError::InvalidCommandParam("RCPT", "no forward path present")
        ));

        // Invalid forward path prefix
        let result = RecipientParam::parse(b"<user@example.com>").unwrap_err();
        assert!(matches!(
            result,
            CommandLineError::InvalidCommandParam("RCPT", "invalid forward path prefix")
        ));

        let result = RecipientParam::parse(b"FROM:<user@example.com>").unwrap_err();
        assert!(matches!(
            result,
            CommandLineError::InvalidCommandParam("RCPT", "invalid forward path prefix")
        ));

        let result = RecipientParam::parse(b"TO:user@example.com>").unwrap_err();
        assert!(matches!(
            result,
            CommandLineError::InvalidCommandParam("RCPT", "invalid forward path prefix")
        ));

        let result = RecipientParam::parse(b"TO:<user@example.com").unwrap_err();
        assert!(matches!(
            result,
            CommandLineError::InvalidCommandParam("RCPT", "invalid forward path prefix")
        ));
    }
}
