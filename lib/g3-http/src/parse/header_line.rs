/*
 * SPDX-License-Identifier: Apache-2.0
 * Copyright 2023-2025 ByteDance and/or its affiliates.
 */

use super::HttpLineParseError;

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
