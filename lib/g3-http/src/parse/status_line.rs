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

use atoi::FromRadix10;

use super::HttpLineParseError;

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
        let s = HttpStatusLine::parse(b"HTTP/1.1 200 OK\r\n").unwrap();
        assert_eq!(s.version, 1);
        assert_eq!(s.code, 200);
        assert_eq!(s.reason, "OK");
    }

    #[test]
    fn no_reason() {
        let s = HttpStatusLine::parse(b"HTTP/1.1 200\r\n").unwrap();
        assert_eq!(s.version, 1);
        assert_eq!(s.code, 200);
        assert_eq!(s.reason, "");
    }
}
