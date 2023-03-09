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

use std::io;

use atoi::FromRadix10Checked;
use tokio::io::{AsyncBufRead, AsyncReadExt};

use g3_io_ext::LimitedBufReadExt;

const SIZE_STRING_MAX_SIZE: usize = 30;

pub struct NetStringParser {
    max_len: usize,

    len: usize,
}

impl NetStringParser {
    /// [max_len] should be small enough as we read to stack
    pub fn new(max_len: usize) -> Self {
        NetStringParser { max_len, len: 0 }
    }

    /// return UnexpectedEof io error if eof
    pub async fn parse<R>(&mut self, reader: &mut R) -> io::Result<String>
    where
        R: AsyncBufRead + Unpin,
    {
        self.len = 0;
        let mut size_buf = Vec::with_capacity(SIZE_STRING_MAX_SIZE);
        let (found, len) = reader
            .limited_read_until(b':', SIZE_STRING_MAX_SIZE, &mut size_buf)
            .await?;
        if len == 0 {
            return Err(io::Error::new(io::ErrorKind::UnexpectedEof, "eof"));
        }
        if !found {
            return if len < SIZE_STRING_MAX_SIZE {
                Err(io::Error::new(io::ErrorKind::UnexpectedEof, "eof"))
            } else {
                Err(io::Error::new(
                    io::ErrorKind::InvalidData,
                    "no size string part found",
                ))
            };
        }
        match usize::from_radix_10_checked(size_buf.as_slice()) {
            (Some(parsed_size), used_len) => {
                if used_len != len - 1 {
                    return Err(io::Error::new(
                        io::ErrorKind::InvalidData,
                        "invalid chars found in size string",
                    ));
                }
                self.len = parsed_size;
            }
            (None, _) => {
                return Err(io::Error::new(
                    io::ErrorKind::InvalidData,
                    "no valid size found in size string",
                ))
            }
        }

        let ret = if self.len > 0 {
            if self.len > self.max_len {
                return Err(io::Error::new(
                    io::ErrorKind::InvalidData,
                    "too large string size",
                ));
            }

            let mut buf = vec![0u8; self.len];
            let _nr = reader.read_exact(&mut buf).await?;
            String::from_utf8(buf).map_err(|e| {
                io::Error::new(
                    io::ErrorKind::InvalidData,
                    format!("not a valid utf-8 string: {e}"),
                )
            })?
        } else {
            String::new()
        };

        let ending = reader.read_u8().await?;
        if ending != b',' {
            return Err(io::Error::new(
                io::ErrorKind::InvalidData,
                "invalid ending character",
            ));
        }

        Ok(ret)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use bytes::Bytes;
    use tokio::io::{AsyncRead, BufReader, Result};
    use tokio_util::io::StreamReader;

    fn to_buf_stream(content: &'static [u8]) -> BufReader<impl AsyncRead> {
        let stream = tokio_stream::iter(vec![Result::Ok(Bytes::from_static(content))]);
        let stream = StreamReader::new(stream);
        BufReader::new(stream)
    }

    #[tokio::test]
    async fn read_single() {
        let mut parser = NetStringParser::new(1024);
        let content = b"7:abcdefg,";
        let mut buf_stream = to_buf_stream(content);

        let s = parser.parse(&mut buf_stream).await.unwrap();
        assert_eq!(s, "abcdefg");
    }

    #[tokio::test]
    async fn read_split() {
        let mut parser = NetStringParser::new(1024);
        let content1 = b"9:abcdefg";
        let content2 = b"hi,";
        let stream = tokio_stream::iter(vec![
            Result::Ok(Bytes::from_static(content1)),
            Result::Ok(Bytes::from_static(content2)),
        ]);
        let stream = StreamReader::new(stream);
        let mut buf_stream = BufReader::new(stream);

        let s = parser.parse(&mut buf_stream).await.unwrap();
        assert_eq!(s, "abcdefghi");
    }

    #[tokio::test]
    async fn read_no_header() {
        let mut parser = NetStringParser::new(1024);
        let content = b"7abcdefg";
        let mut buf_stream = to_buf_stream(content);

        assert!(parser.parse(&mut buf_stream).await.is_err());
    }

    #[tokio::test]
    async fn read_no_trailer() {
        let mut parser = NetStringParser::new(1024);
        let content = b"7:abcdefg";
        let mut buf_stream = to_buf_stream(content);

        assert!(parser.parse(&mut buf_stream).await.is_err());
    }

    #[tokio::test]
    async fn read_too_long() {
        let mut parser = NetStringParser::new(6);
        let content = b"7:abcdefg";
        let mut buf_stream = to_buf_stream(content);

        assert!(parser.parse(&mut buf_stream).await.is_err());
    }

    #[tokio::test]
    async fn read_too_much_long() {
        let mut parser = NetStringParser::new(1024);
        let content = b"11111111112222222222333333333344444444:abcdefg";
        let mut buf_stream = to_buf_stream(content);

        assert!(parser.parse(&mut buf_stream).await.is_err());
    }
}
