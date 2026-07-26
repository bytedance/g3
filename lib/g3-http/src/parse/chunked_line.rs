/*
 * SPDX-License-Identifier: Apache-2.0
 * SPDX-FileCopyrightText: 2023-2025 ByteDance and/or its affiliates.
 * SPDX-FileCopyrightText: 2026 VEY-OSS Developers.
 */

use atoi::FromRadix16Checked;

use super::HttpLineParseError;

#[derive(Debug)]
pub struct HttpChunkedLine<'a> {
    pub chunk_size: u64,
    pub extension: Option<&'a str>,
}

impl<'a> HttpChunkedLine<'a> {
    pub fn parse(buf: &'a [u8]) -> Result<HttpChunkedLine<'a>, HttpLineParseError> {
        let (chunk_size, offset) = u64::from_radix_16_checked(buf);
        let Some(chunk_size) = chunk_size else {
            return Err(HttpLineParseError::InvalidChunkSize);
        };
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

    #[test]
    fn reject_overflowing_chunk_size() {
        let err = HttpChunkedLine::parse(b"10000000000000000\r\n").unwrap_err();
        assert!(matches!(err, HttpLineParseError::InvalidChunkSize));

        let err = HttpChunkedLine::parse(b"1000000000000000f\r\n").unwrap_err();
        assert!(matches!(err, HttpLineParseError::InvalidChunkSize));
    }
}
