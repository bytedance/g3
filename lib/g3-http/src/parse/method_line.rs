/*
 * Copyright 2023 ByteDance and/or its affiliates.
 *
 * Licensed under the Apache License, Version 2.0 (the "License");
 * you may not use this file except in compliance with the License.
 * You may obtain a copy of the License at
 *
 *     http://www.apache.org/licenses/LICENSE-2.0
 *
 * Unless required by applicable law or agreed to in writing, software
 * distributed under the License is distributed on an "AS IS" BASIS,
 * WITHOUT WARRANTIES OR CONDITIONS OF ANY KIND, either express or implied.
 * See the License for the specific language governing permissions and
 * limitations under the License.
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
