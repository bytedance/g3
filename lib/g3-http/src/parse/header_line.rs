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
