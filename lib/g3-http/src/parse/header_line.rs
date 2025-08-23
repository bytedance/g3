/*
 * SPDX-License-Identifier: Apache-2.0
 * Copyright 2023-2025 ByteDance and/or its affiliates.
 */

use super::HttpLineParseError;

#[derive(Debug)]
pub struct HttpHeaderLine<'a> {
    pub name: &'a str,
    pub value: &'a str,
}

impl<'a> HttpHeaderLine<'a> {
    pub fn parse(buf: &'a [u8]) -> Result<HttpHeaderLine<'a>, HttpLineParseError> {
        let line = std::str::from_utf8(buf)?;
        let Some(p) = memchr::memchr(b':', line.as_bytes()) else {
            return Err(HttpLineParseError::NoDelimiterFound(':'));
        };

        let name = line[0..p].trim();
        let value = line[p + 1..].trim();

        Ok(HttpHeaderLine { name, value })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parse_header_line_ok() {
        // Standard key-value pair
        let valid = b"Content-Type: application/json";
        let header = HttpHeaderLine::parse(valid).unwrap();
        assert_eq!(header.name, "Content-Type");
        assert_eq!(header.value, "application/json");

        // Key-value with surrounding spaces
        let with_spaces = b"  Accept  :  */*  ";
        let header = HttpHeaderLine::parse(with_spaces).unwrap();
        assert_eq!(header.name, "Accept");
        assert_eq!(header.value, "*/*");
    }

    #[test]
    fn parse_header_line_err() {
        // Invalid UTF-8 sequence
        let invalid_utf8 = b"name\xff: value";
        let err = HttpHeaderLine::parse(invalid_utf8).unwrap_err();
        assert!(matches!(err, HttpLineParseError::InvalidUtf8Encoding(_)));

        // Missing colon delimiter
        let no_colon = b"name value";
        let err = HttpHeaderLine::parse(no_colon).unwrap_err();
        assert!(matches!(err, HttpLineParseError::NoDelimiterFound(':')));
    }
}
