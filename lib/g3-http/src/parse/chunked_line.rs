/*
 * SPDX-License-Identifier: Apache-2.0
 * Copyright 2023-2025 ByteDance and/or its affiliates.
 */

use atoi::FromRadix16;

use super::HttpLineParseError;

#[derive(Debug)]
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

    #[test]
    fn empty_input() {
        let err = HttpChunkedLine::parse(b"").unwrap_err();
        assert!(matches!(err, HttpLineParseError::InvalidChunkSize));
    }

    #[test]
    fn no_suffix() {
        let err = HttpChunkedLine::parse(b"1").unwrap_err();
        assert!(matches!(err, HttpLineParseError::NotLongEnough));
    }

    #[test]
    fn invalid_suffix() {
        let err = HttpChunkedLine::parse(b"1X").unwrap_err();
        assert!(matches!(err, HttpLineParseError::InvalidChunkSize));
    }

    #[test]
    fn non_utf8_extension() {
        let err = HttpChunkedLine::parse(b"1;\xFF").unwrap_err();
        assert!(matches!(err, HttpLineParseError::InvalidUtf8Encoding(_)));
    }
}
