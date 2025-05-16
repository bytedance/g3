/*
 * SPDX-License-Identifier: Apache-2.0
 * Copyright 2024-2025 ByteDance and/or its affiliates.
 */

use std::io;
use std::pin::Pin;
use std::task::{Context, Poll, ready};

use tokio::io::{AsyncBufRead, AsyncRead, ReadBuf};

use g3_types::net::HttpHeaderMap;

use crate::{
    ChunkedDataDecodeReader, HttpBodyReader, HttpBodyType, TrailerReadError, TrailerReader,
};

enum HttpBodyDecodeState<'a, R> {
    Plain(HttpBodyReader<'a, R>),
    Chunked(ChunkedDataDecodeReader<'a, R>),
}

pub struct HttpBodyDecodeReader<'a, R> {
    read_data_done: bool,
    finished: bool,
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

    pub fn new_read_until_end(stream: &'a mut R) -> Self {
        HttpBodyDecodeReader::with_state(HttpBodyDecodeState::Plain(
            HttpBodyReader::new_read_until_end(stream),
        ))
    }

    pub fn new_fixed_length(stream: &'a mut R, content_length: u64) -> Self {
        HttpBodyDecodeReader::with_state(HttpBodyDecodeState::Plain(
            HttpBodyReader::new_fixed_length(stream, content_length),
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

        match state {
            HttpBodyDecodeState::Plain(_) => Ok(None),
            HttpBodyDecodeState::Chunked(decoder) => {
                let headers = TrailerReader::new(decoder.into_reader(), max_size).await?;
                self.finished = true;
                if headers.is_empty() {
                    Ok(None)
                } else {
                    Ok(Some(headers))
                }
            }
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

        let Some(reader) = self.decode_state.as_mut() else {
            return Poll::Ready(Ok(()));
        };

        let prev_len = buf.filled().len();
        match reader {
            HttpBodyDecodeState::Plain(r) => {
                ready!(Pin::new(r).poll_read(cx, buf))?;
                if buf.filled().len() == prev_len {
                    self.read_data_done = true;
                    self.finished = true;
                }
            }
            HttpBodyDecodeState::Chunked(c) => {
                ready!(Pin::new(c).poll_read(cx, buf))?;
                if buf.filled().len() == prev_len {
                    self.read_data_done = true;
                }
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
