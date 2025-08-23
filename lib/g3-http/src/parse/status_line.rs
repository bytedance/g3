/*
 * SPDX-License-Identifier: Apache-2.0
 * Copyright 2023-2025 ByteDance and/or its affiliates.
 */

use atoi::FromRadix10;

use super::HttpLineParseError;

#[derive(Debug)]
pub struct HttpStatusLine<'a> {
    pub version: u8,
    pub code: u16,
    pub reason: &'a str,
}

impl<'a> HttpStatusLine<'a> {
    pub fn parse(buf: &'a [u8]) -> Result<HttpStatusLine<'a>, HttpLineParseError> {
        const MINIMAL_LENGTH: usize = 13; // HTTP/1.x XYZ\n

        if buf.len() < MINIMAL_LENGTH {
            return Err(HttpLineParseError::NotLongEnough);
        }

        let Some(p) = memchr::memchr(b' ', buf) else {
            return Err(HttpLineParseError::NoDelimiterFound(' '));
        };
        let version: u8 = match &buf[0..p] {
            b"HTTP/1.0" => 0,
            b"HTTP/1.1" => 1,
            b"HTTP/2.0" | b"HTTP/2" => 2,
            _ => return Err(HttpLineParseError::InvalidVersion),
        };

        let left = &buf[p + 1..];
        let (code, len) = u16::from_radix_10(left);
        if len < 3 {
            return Err(HttpLineParseError::InvalidStatusCode);
        }

        if left.len() < len + 1 {
            return Err(HttpLineParseError::NotLongEnough);
        }
        let reason = std::str::from_utf8(&left[len + 1..])?.trim();

        Ok(HttpStatusLine {
            version,
            code,
            reason,
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn normal() {
        let s = HttpStatusLine::parse(b"HTTP/1.0 200 OK\r\n").unwrap();
        assert_eq!(s.version, 0);
        assert_eq!(s.code, 200);
        assert_eq!(s.reason, "OK");
    }

    #[test]
    fn no_reason() {
        let s = HttpStatusLine::parse(b"HTTP/1.1 200 \r\n").unwrap();
        assert_eq!(s.version, 1);
        assert_eq!(s.code, 200);
        assert_eq!(s.reason, "");
    }

    #[test]
    fn no_reason_no_sp() {
        let s = HttpStatusLine::parse(b"HTTP/2.0 200\r\n").unwrap();
        assert_eq!(s.version, 2);
        assert_eq!(s.code, 200);
        assert_eq!(s.reason, "");
    }

    #[test]
    fn not_long_enough() {
        let e = HttpStatusLine::parse(b"HTTP/1.1").unwrap_err();
        assert!(matches!(e, HttpLineParseError::NotLongEnough));
    }

    #[test]
    fn no_delimiter_found() {
        let e = HttpStatusLine::parse(b"HTTP/1.1200OK\r\n").unwrap_err();
        assert!(matches!(e, HttpLineParseError::NoDelimiterFound(' ')));
    }

    #[test]
    fn invalid_version() {
        let e = HttpStatusLine::parse(b"HTTP/3.0 200 OK\r\n").unwrap_err();
        assert!(matches!(e, HttpLineParseError::InvalidVersion));
    }

    #[test]
    fn invalid_status_code() {
        let e = HttpStatusLine::parse(b"HTTP/1.1 20 OK\r\n").unwrap_err();
        assert!(matches!(e, HttpLineParseError::InvalidStatusCode));
    }

    #[test]
    fn not_long_enough_after_code() {
        let e = HttpStatusLine::parse(b"HTTP/1.1 200").unwrap_err();
        assert!(matches!(e, HttpLineParseError::NotLongEnough));
    }

    #[test]
    fn invalid_utf8_encoding() {
        let e = HttpStatusLine::parse(b"HTTP/1.1 200 OK\xFF\xFF\r\n").unwrap_err();
        assert!(matches!(e, HttpLineParseError::InvalidUtf8Encoding(_)));
    }
}
