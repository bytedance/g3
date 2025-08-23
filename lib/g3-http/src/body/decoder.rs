/*
 * SPDX-License-Identifier: Apache-2.0
 * Copyright 2024-2025 ByteDance and/or its affiliates.
 */

use std::io;
use std::pin::Pin;
use std::task::{Context, Poll, ready};

use tokio::io::{AsyncBufRead, AsyncRead, ReadBuf};

use g3_types::net::HttpHeaderMap;

use crate::{ChunkedDataDecodeReader, HttpBodyType, TrailerReadError, TrailerReader};

enum HttpBodyDecodeState<'a, R> {
    ReadUntilEnd(&'a mut R),
    ReadFixedLength(&'a mut R, u64),
    Chunked(ChunkedDataDecodeReader<'a, R>),
}

pub struct HttpBodyDecodeReader<'a, R> {
    read_data_done: bool,
    finished: bool,
    total_read: u64,
    decode_state: Option<HttpBodyDecodeState<'a, R>>,
}

impl<'a, R> HttpBodyDecodeReader<'a, R>
where
    R: AsyncBufRead + Unpin,
{
    fn with_state(state: HttpBodyDecodeState<'a, R>) -> Self {
        HttpBodyDecodeReader {
            read_data_done: false,
            finished: false,
            total_read: 0,
            decode_state: Some(state),
        }
    }

    pub fn new(stream: &'a mut R, body_type: HttpBodyType, body_line_max_size: usize) -> Self {
        match body_type {
            HttpBodyType::ReadUntilEnd => Self::new_read_until_end(stream),
            HttpBodyType::ContentLength(size) => Self::new_fixed_length(stream, size),
            HttpBodyType::Chunked => Self::new_chunked(stream, body_line_max_size),
        }
    }

    pub fn new_read_until_end(reader: &'a mut R) -> Self {
        HttpBodyDecodeReader::with_state(HttpBodyDecodeState::ReadUntilEnd(reader))
    }

    pub fn new_fixed_length(reader: &'a mut R, content_length: u64) -> Self {
        HttpBodyDecodeReader::with_state(HttpBodyDecodeState::ReadFixedLength(
            reader,
            content_length,
        ))
    }

    pub fn new_chunked(stream: &'a mut R, body_line_max_size: usize) -> Self {
        HttpBodyDecodeReader::with_state(HttpBodyDecodeState::Chunked(
            ChunkedDataDecodeReader::new(stream, body_line_max_size),
        ))
    }

    pub async fn trailer(
        &mut self,
        max_size: usize,
    ) -> Result<Option<HttpHeaderMap>, TrailerReadError> {
        if !self.read_data_done {
            return Err(TrailerReadError::ReadError(io::Error::other(
                "data has not been read out yet",
            )));
        }
        if self.finished {
            return Ok(None);
        }
        let Some(state) = self.decode_state.take() else {
            return Ok(None);
        };

        if let HttpBodyDecodeState::Chunked(decoder) = state {
            let headers = TrailerReader::new(decoder.into_reader(), max_size).await?;
            self.finished = true;
            if headers.is_empty() {
                Ok(None)
            } else {
                Ok(Some(headers))
            }
        } else {
            Ok(None)
        }
    }

    pub fn finished(&self) -> bool {
        self.finished
    }
}

impl<R> AsyncRead for HttpBodyDecodeReader<'_, R>
where
    R: AsyncBufRead + Unpin,
{
    fn poll_read(
        mut self: Pin<&mut Self>,
        cx: &mut Context<'_>,
        buf: &mut ReadBuf<'_>,
    ) -> Poll<io::Result<()>> {
        if self.read_data_done {
            return Poll::Ready(Ok(()));
        }
        if buf.remaining() == 0 {
            // invalid read action
            return Poll::Ready(Ok(()));
        }

        let total_read = self.total_read;
        let Some(reader) = self.decode_state.as_mut() else {
            return Poll::Ready(Ok(()));
        };

        match reader {
            HttpBodyDecodeState::ReadUntilEnd(r) => {
                let prev_len = buf.filled().len();
                ready!(Pin::new(r).poll_read(cx, buf))?;
                let nr = buf.filled().len() - prev_len;
                if buf.filled().len() == prev_len {
                    self.read_data_done = true;
                    self.finished = true;
                }
                self.total_read += nr as u64;
            }
            HttpBodyDecodeState::ReadFixedLength(r, max_len) => {
                let max_read = *max_len;
                let left = max_read - total_read;
                let to_read = left.min(buf.remaining() as u64) as usize;
                let mut new_buf = ReadBuf::new(buf.initialize_unfilled_to(to_read));
                ready!(Pin::new(r).poll_read(cx, &mut new_buf))?;
                let nr = new_buf.filled().len();
                if nr == 0 {
                    return Poll::Ready(Err(io::Error::new(
                        io::ErrorKind::UnexpectedEof,
                        format!("EOF after read {total_read} of {max_read} body"),
                    )));
                }
                buf.advance(nr);
                self.total_read += nr as u64;
                if self.total_read == max_read {
                    self.read_data_done = true;
                    self.finished = true;
                }
            }
            HttpBodyDecodeState::Chunked(c) => {
                let prev_len = buf.filled().len();
                let mut pin_c = Pin::new(c);
                ready!(pin_c.as_mut().poll_read(cx, buf))?;
                if pin_c.finished() {
                    self.read_data_done = true;
                }
                let nr = buf.filled().len() - prev_len;
                self.total_read += nr as u64;
            }
        }
        Poll::Ready(Ok(()))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use tokio::io::{AsyncReadExt, BufReader};

    #[tokio::test]
    async fn read_single_to_end() {
        let content = b"test body";
        let stream = tokio_test::io::Builder::new().read(content).build();
        let mut buf_stream = BufReader::new(stream);
        let mut body_reader = HttpBodyDecodeReader::new_read_until_end(&mut buf_stream);

        let mut buf = [0u8; 16];
        let len = body_reader.read(&mut buf).await.unwrap();
        assert_eq!(len, content.len());
        assert_eq!(&buf[0..len], content);
        let len = body_reader.read(&mut buf).await.unwrap();
        assert_eq!(len, 0);
        assert!(body_reader.finished());
    }

    #[tokio::test]
    async fn read_split_to_end() {
        let content1 = b"test body";
        let content2 = b"hello world";
        let stream = tokio_test::io::Builder::new()
            .read(content1)
            .read(content2)
            .build();
        let mut buf_stream = BufReader::new(stream);
        let mut body_reader = HttpBodyDecodeReader::new_read_until_end(&mut buf_stream);

        let mut buf = [0u8; 32];
        let len = body_reader.read(&mut buf).await.unwrap();
        assert_eq!(len, content1.len());
        assert_eq!(&buf[0..len], content1);
        let len = body_reader.read(&mut buf).await.unwrap();
        assert_eq!(len, content2.len());
        assert_eq!(&buf[0..len], content2);
        let len = body_reader.read(&mut buf).await.unwrap();
        assert_eq!(len, 0);
        assert!(body_reader.finished());
    }

    #[tokio::test]
    async fn read_single_content_length() {
        let body_len: usize = 9;
        let content = b"test bodyxxxx";
        let stream = tokio_test::io::Builder::new().read(content).build();
        let mut buf_stream = BufReader::new(stream);
        let mut body_reader =
            HttpBodyDecodeReader::new_fixed_length(&mut buf_stream, body_len as u64);

        let mut buf = [0u8; 16];
        let len = body_reader.read(&mut buf).await.unwrap();
        assert_eq!(len, body_len);
        assert_eq!(&buf[0..len], &content[0..len]);
        let len = body_reader.read(&mut buf).await.unwrap();
        assert_eq!(len, 0);
        assert!(body_reader.finished());
    }

    #[tokio::test]
    async fn read_split_content_length() {
        let body_len: usize = 20;
        let content1 = b"hello world";
        let content2 = b"test bodyxxxx";
        let stream = tokio_test::io::Builder::new()
            .read(content1)
            .read(content2)
            .build();
        let mut buf_stream = BufReader::new(stream);
        let mut body_reader =
            HttpBodyDecodeReader::new_fixed_length(&mut buf_stream, body_len as u64);

        let mut buf = [0u8; 32];
        let len = body_reader.read(&mut buf).await.unwrap();
        assert_eq!(len, content1.len());
        assert_eq!(&buf[0..len], content1);
        let len = body_reader.read(&mut buf).await.unwrap();
        assert_eq!(len, body_len - content1.len());
        assert_eq!(&buf[0..len], &content2[0..len]);
        let len = body_reader.read(&mut buf).await.unwrap();
        assert_eq!(len, 0);
        assert!(body_reader.finished());
    }

    #[tokio::test]
    async fn read_empty_chunked() {
        let body_len: usize = 0;
        let content = b"0\r\n\r\n";
        let stream = tokio_test::io::Builder::new().read(content).build();
        let mut buf_stream = BufReader::new(stream);
        let mut body_reader = HttpBodyDecodeReader::new_chunked(&mut buf_stream, 1024);

        let mut buf = Vec::with_capacity(32);
        tokio::io::copy(&mut body_reader, &mut buf).await.unwrap();
        assert_eq!(buf.len(), body_len);
        assert!(!body_reader.finished());
        let header = body_reader.trailer(1024).await.unwrap();
        assert!(header.is_none());
        assert!(body_reader.finished());
    }

    #[tokio::test]
    async fn read_single_chunked() {
        let body_len: usize = 9;
        let content = b"5\r\ntest\n\r\n4\r\nbody\r\n0\r\n\r\nXXX";
        let stream = tokio_test::io::Builder::new().read(content).build();
        let mut buf_stream = BufReader::new(stream);
        let mut body_reader = HttpBodyDecodeReader::new_chunked(&mut buf_stream, 1024);

        let mut buf = Vec::with_capacity(32);
        tokio::io::copy(&mut body_reader, &mut buf).await.unwrap();
        assert_eq!(buf.len(), body_len);
        assert_eq!(&buf, b"test\nbody");
        assert!(!body_reader.finished());
        let header = body_reader.trailer(1024).await.unwrap();
        assert!(header.is_none());
        assert!(body_reader.finished());
    }

    #[tokio::test]
    async fn read_split_chunked() {
        let body_len: usize = 9;
        let content1 = b"5\r\ntest\n\r\n4\r";
        let content2 = b"\nbody\r\n0\r\n\r\nXXX";
        let stream = tokio_test::io::Builder::new()
            .read(content1)
            .read(content2)
            .build();
        let mut buf_stream = BufReader::new(stream);
        let mut body_reader = HttpBodyDecodeReader::new_chunked(&mut buf_stream, 1024);

        let mut buf = Vec::with_capacity(32);
        tokio::io::copy(&mut body_reader, &mut buf).await.unwrap();
        assert_eq!(buf.len(), body_len);
        assert_eq!(&buf, b"test\nbody");
        assert!(!body_reader.finished());
        let header = body_reader.trailer(1024).await.unwrap();
        assert!(header.is_none());
        assert!(body_reader.finished());
    }

    #[tokio::test]
    async fn read_long_chunked() {
        let content1 = b"5\r\ntest\n\r\n";
        let content2 = b"4\r\nbody\r\n";
        let content3 = b"20\r\naabbbbbbbbbbccccccccccdddddddddd\r\n";
        let content4 = b"0\r\n\r\nXXX";
        let stream = tokio_test::io::Builder::new()
            .read(content1)
            .read(content2)
            .read(content3)
            .read(content4)
            .build();
        let mut buf_stream = BufReader::new(stream);
        let mut body_reader =
            HttpBodyDecodeReader::new(&mut buf_stream, HttpBodyType::Chunked, 1024);

        let mut buf = [0u8; 32];
        let len = body_reader.read(&mut buf).await.unwrap();
        assert_eq!(len, buf.len());
        assert_eq!(buf.as_slice(), b"test\nbodyaabbbbbbbbbbccccccccccd");
        assert!(!body_reader.finished());

        let len = body_reader.read(&mut buf).await.unwrap();
        assert_eq!(len, 9);
        assert_eq!(&buf[..len], b"ddddddddd");
        assert!(!body_reader.finished());

        let header = body_reader.trailer(1024).await.unwrap();
        assert!(header.is_none());
        assert!(body_reader.finished());
    }

    #[tokio::test]
    async fn read_single_trailer() {
        let body_len: usize = 9;
        let content = b"5\r\ntest\n\r\n4\r\nbody\r\n0\r\nA: B\r\n\r\nXX";
        let stream = tokio_test::io::Builder::new().read(content).build();
        let mut buf_stream = BufReader::new(stream);
        let mut body_reader = HttpBodyDecodeReader::new_chunked(&mut buf_stream, 1024);

        let mut buf = Vec::with_capacity(32);
        tokio::io::copy(&mut body_reader, &mut buf).await.unwrap();
        assert_eq!(buf.len(), body_len);
        assert_eq!(&buf, b"test\nbody");
        assert!(!body_reader.finished());
        let header = body_reader.trailer(1024).await.unwrap();
        assert!(header.is_some());
        assert!(body_reader.finished());

        let headers = header.unwrap();
        let v = headers.get("a").unwrap();
        assert_eq!(v.as_bytes(), b"B");
    }
}
