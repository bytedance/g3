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

use atoi::FromRadix16;

use super::HttpLineParseError;

pub struct HttpChunkedLine<'a> {
    pub chunk_size: u64,
    pub extension: Option<&'a str>,
}

impl<'a> HttpChunkedLine<'a> {
    pub fn parse(buf: &'a [u8]) -> Result<HttpChunkedLine<'a>, HttpLineParseError> {
        let (chunk_size, offset) = u64::from_radix_16(buf);
        if offset == 0 {
            return Err(HttpLineParseError::InvalidChunkSize);
        }

        if buf.len() == offset {
            return Err(HttpLineParseError::NotLongEnough);
        }

        match buf[offset] {
            b'\r' | b'\n' => Ok(HttpChunkedLine {
                chunk_size,
                extension: None,
            }),
            b';' => {
                let extension = std::str::from_utf8(&buf[offset + 1..])
                    .map_err(HttpLineParseError::InvalidUtf8Encoding)?
                    .trim();
                Ok(HttpChunkedLine {
                    chunk_size,
                    extension: Some(extension),
                })
            }
            _ => Err(HttpLineParseError::InvalidChunkSize),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn simple() {
        let chunk = HttpChunkedLine::parse(b"1\r\n").unwrap();
        assert_eq!(chunk.chunk_size, 1);

        let chunk = HttpChunkedLine::parse(b"1F\r\n").unwrap();
        assert_eq!(chunk.chunk_size, 0x1f);
    }

    #[test]
    fn with_extension() {
        let chunk = HttpChunkedLine::parse(b"1; ieof\r\n").unwrap();
        assert_eq!(chunk.chunk_size, 1);
        assert_eq!(chunk.extension, Some("ieof"));
    }
}
