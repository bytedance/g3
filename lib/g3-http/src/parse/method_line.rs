/*
 * SPDX-License-Identifier: Apache-2.0
 * Copyright 2023-2025 ByteDance and/or its affiliates.
 */

use super::HttpLineParseError;

#[derive(Debug)]
pub struct HttpMethodLine<'a> {
    pub version: u8,
    pub method: &'a str,
    pub uri: &'a str,
}

impl<'a> HttpMethodLine<'a> {
    pub fn parse(buf: &'a [u8]) -> Result<HttpMethodLine<'a>, HttpLineParseError> {
        let line = std::str::from_utf8(buf)?;

        let Some(p1) = memchr::memchr(b' ', line.as_bytes()) else {
            return Err(HttpLineParseError::NoDelimiterFound(' '));
        };

        let method = &line[0..p1];
        let left = &line[p1 + 1..];

        let Some(p2) = memchr::memchr(b' ', left.as_bytes()) else {
            return Err(HttpLineParseError::NoDelimiterFound(' '));
        };
        let uri = left[0..p2].trim();

        let version: u8 = match left[p2 + 1..].trim() {
            "HTTP/1.0" => 0,
            "HTTP/1.1" => 1,
            "HTTP/2.0" | "HTTP/2" => 2,
            _ => return Err(HttpLineParseError::InvalidVersion),
        };

        Ok(HttpMethodLine {
            version,
            method,
            uri,
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parse_valid_method_line() {
        // HTTP/1.1
        let data = b"GET /index.html HTTP/1.1";
        let result = HttpMethodLine::parse(data).unwrap();
        assert_eq!(result.method, "GET");
        assert_eq!(result.uri, "/index.html");
        assert_eq!(result.version, 1);

        // HTTP/1.0
        let data = b"POST /api HTTP/1.0";
        let result = HttpMethodLine::parse(data).unwrap();
        assert_eq!(result.method, "POST");
        assert_eq!(result.uri, "/api");
        assert_eq!(result.version, 0);

        // HTTP/2
        let data = b"OPTIONS * HTTP/2";
        let result = HttpMethodLine::parse(data).unwrap();
        assert_eq!(result.method, "OPTIONS");
        assert_eq!(result.uri, "*");
        assert_eq!(result.version, 2);

        // HTTP/2.0
        let data = b"GET / HTTP/2.0";
        let result = HttpMethodLine::parse(data).unwrap();
        assert_eq!(result.version, 2);
    }

    #[test]
    fn invalid_utf8() {
        let data = b"GET /\xFF HTTP/1.1";
        let err = HttpMethodLine::parse(data).unwrap_err();
        assert!(matches!(err, HttpLineParseError::InvalidUtf8Encoding(_)));
    }

    #[test]
    fn missing_first_space() {
        let data = b"GET/index.html HTTP/1.1";
        let err = HttpMethodLine::parse(data).unwrap_err();
        assert!(matches!(err, HttpLineParseError::NoDelimiterFound(' ')));
    }

    #[test]
    fn missing_second_space() {
        let data = b"GET /index.htmlHTTP/1.1";
        let err = HttpMethodLine::parse(data).unwrap_err();
        assert!(matches!(err, HttpLineParseError::NoDelimiterFound(' ')));
    }

    #[test]
    fn invalid_version() {
        let data = b"GET / HTP/1.1";
        let err = HttpMethodLine::parse(data).unwrap_err();
        assert!(matches!(err, HttpLineParseError::InvalidVersion));

        let data = b"GET / HTTP/3.0";
        let err = HttpMethodLine::parse(data).unwrap_err();
        assert!(matches!(err, HttpLineParseError::InvalidVersion));
    }

    #[test]
    fn empty_input() {
        let data = b"";
        let err = HttpMethodLine::parse(data).unwrap_err();
        assert!(matches!(err, HttpLineParseError::NoDelimiterFound(' ')));
    }
}
