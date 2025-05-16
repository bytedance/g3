/*
 * SPDX-License-Identifier: Apache-2.0
 * Copyright 2023-2025 ByteDance and/or its affiliates.
 */

use atoi::FromRadix10;

use super::IcapLineParseError;

pub(crate) struct StatusLine<'a> {
    pub(crate) code: u16,
    pub(crate) message: &'a str,
}

impl<'a> StatusLine<'a> {
    pub(crate) fn parse(buf: &'a [u8]) -> Result<StatusLine<'a>, IcapLineParseError> {
        const PREFIX: &str = "ICAP/1.0 ";
        const MINIMAL_LENGTH: usize = 13; // ICAP/1.0 XYZ\n

        if buf.len() < MINIMAL_LENGTH {
            return Err(IcapLineParseError::NotLongEnough);
        }
        if !buf.starts_with(PREFIX.as_bytes()) {
            return Err(IcapLineParseError::InvalidIcapVersion);
        }

        let left = &buf[PREFIX.len()..];
        let (code, len) = u16::from_radix_10(left);
        if len != 3 || !(100..600).contains(&code) {
            return Err(IcapLineParseError::InvalidStatusCode);
        }

        if left.len() < len + 1 {
            return Err(IcapLineParseError::NotLongEnough);
        }
        let message = std::str::from_utf8(&left[len + 1..])?.trim();

        Ok(StatusLine { code, message })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn normal() {
        let status = StatusLine::parse(b"ICAP/1.0 200 OK\r\n").unwrap();
        assert_eq!(status.code, 200);
        assert_eq!(status.message, "OK");
    }

    #[test]
    fn no_reason() {
        let status = StatusLine::parse(b"ICAP/1.0 200\r\n").unwrap();
        assert_eq!(status.code, 200);
        assert_eq!(status.message, "");
    }
}
