/*
 * SPDX-License-Identifier: Apache-2.0
 * Copyright 2023-2025 ByteDance and/or its affiliates.
 */

use super::HttpLineParseError;

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
