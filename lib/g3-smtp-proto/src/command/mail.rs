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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn mail_param_parse() {
        // Valid
        let result = MailParam::parse(b"FROM:<user@example.com>").unwrap();
        assert_eq!(result.reverse_path(), "<user@example.com>");

        let result = MailParam::parse(b"From:<admin@domain.org>").unwrap();
        assert_eq!(result.reverse_path(), "<admin@domain.org>");

        let result = MailParam::parse(b"from:<>").unwrap();
        assert_eq!(result.reverse_path(), "<>");

        // Invalid UTF-8 sequences
        let result = MailParam::parse(b"FROM:\xff\xfe<user@example.com>").unwrap_err();
        assert!(matches!(result, CommandLineError::InvalidUtf8Command(_)));

        let result = MailParam::parse(b"FROM:<\xff\xfe>").unwrap_err();
        assert!(matches!(result, CommandLineError::InvalidUtf8Command(_)));

        // No reverse path present
        let result = MailParam::parse(b"   ").unwrap_err();
        assert!(matches!(
            result,
            CommandLineError::InvalidCommandParam("MAIL", "no reverse path present")
        ));

        // Invalid reverse path prefix
        let result = MailParam::parse(b"<user@example.com>").unwrap_err();
        assert!(matches!(
            result,
            CommandLineError::InvalidCommandParam("MAIL", "invalid reverse path prefix")
        ));

        let result = MailParam::parse(b"TO:<user@example.com>").unwrap_err();
        assert!(matches!(
            result,
            CommandLineError::InvalidCommandParam("MAIL", "invalid reverse path prefix")
        ));

        let result = MailParam::parse(b"FROM:user@example.com>").unwrap_err();
        assert!(matches!(
            result,
            CommandLineError::InvalidCommandParam("MAIL", "invalid reverse path prefix")
        ));

        let result = MailParam::parse(b"FROM:<user@example.com").unwrap_err();
        assert!(matches!(
            result,
            CommandLineError::InvalidCommandParam("MAIL", "invalid reverse path prefix")
        ));
    }
}
