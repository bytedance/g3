/*
 * Copyright 2024 ByteDance and/or its affiliates.
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
use std::pin::Pin;
use std::task::{ready, Context, Poll};

use pin_project_lite::pin_project;
use tokio::io::{AsyncBufRead, AsyncRead, ReadBuf};

struct DataDecodeBuffer {
    buf: Box<[u8]>,
    start: usize,
    end: usize,
    cache_data: Option<(usize, usize)>,
    read_done: bool,
}

impl DataDecodeBuffer {
    fn new(buf_size: usize) -> Self {
        DataDecodeBuffer {
            buf: vec![0; buf_size].into_boxed_slice(),
            start: 0,
            end: 0,
            cache_data: None,
            read_done: false,
        }
    }

    fn poll_fill_buf<R>(
        &mut self,
        cx: &mut Context<'_>,
        reader: Pin<&mut R>,
    ) -> Poll<io::Result<usize>>
    where
        R: AsyncRead + Unpin,
    {
        let mut read_buf = ReadBuf::new(&mut self.buf[self.end..]);
        ready!(reader.poll_read(cx, &mut read_buf))?;
        let nr = read_buf.filled().len();
        self.end += nr;
        Poll::Ready(Ok(nr))
    }

    fn get_line(&self) -> Option<(usize, usize)> {
        if self.start >= self.end {
            return None;
        }
        let src_buf = &self.buf[self.start..self.end];
        if src_buf.is_empty() {
            None
        } else {
            memchr::memchr(b'\n', src_buf).map(|p| (self.start, self.start + p + 1))
        }
    }

    fn poll_line<R>(
        &mut self,
        cx: &mut Context<'_>,
        mut reader: Pin<&mut R>,
    ) -> Poll<io::Result<(usize, usize)>>
    where
        R: AsyncRead + Unpin,
    {
        loop {
            if let Some(v) = self.get_line() {
                return Poll::Ready(Ok(v));
            }

            if self.start > 0 {
                self.buf.copy_within(self.start..self.end, 0);
                self.end -= self.start;
                self.start = 0;
            } else if self.end >= self.buf.len() {
                // line too long
                return Poll::Ready(Err(io::Error::other("line too long")));
            }

            let nr = ready!(self.poll_fill_buf(cx, reader.as_mut()))?;
            if nr == 0 {
                return Poll::Ready(Err(io::Error::new(
                    io::ErrorKind::UnexpectedEof,
                    "connection closed while reading line end",
                )));
            }
        }
    }

    fn poll_read<R>(
        &mut self,
        cx: &mut Context<'_>,
        mut reader: Pin<&mut R>,
        buf: &mut ReadBuf<'_>,
    ) -> Poll<io::Result<()>>
    where
        R: AsyncRead + Unpin,
    {
        if self.read_done {
            return Poll::Ready(Ok(()));
        }

        loop {
            if let Some((data_start, data_end)) = self.cache_data.take() {
                let unfilled = buf.initialize_unfilled();
                let cache = &self.buf[data_start..data_end];
                if cache.len() >= unfilled.len() {
                    let to_copy = unfilled.len();
                    unsafe {
                        std::ptr::copy_nonoverlapping(
                            cache.as_ptr(),
                            unfilled.as_mut_ptr(),
                            to_copy,
                        )
                    };
                    buf.advance(to_copy);
                    self.cache_data = Some((data_start + to_copy, data_end));
                    return Poll::Ready(Ok(()));
                } else {
                    let to_copy = cache.len();
                    unsafe {
                        std::ptr::copy_nonoverlapping(
                            cache.as_ptr(),
                            unfilled.as_mut_ptr(),
                            to_copy,
                        )
                    };
                    buf.advance(to_copy);
                    self.start = data_end;
                }
            }

            let (start, end) = ready!(self.poll_line(cx, reader.as_mut()))?;
            let line = &self.buf[start..end];
            if line[0] == b'.' {
                if line == b".\r\n" {
                    self.read_done = true;
                    return Poll::Ready(Ok(()));
                }
                self.cache_data = Some((start + 1, end));
            } else {
                self.cache_data = Some((start, end));
            }
        }
    }

    fn poll_get_cache<R>(
        &mut self,
        cx: &mut Context<'_>,
        reader: Pin<&mut R>,
    ) -> Poll<io::Result<()>>
    where
        R: AsyncRead + Unpin,
    {
        if self.read_done {
            return Poll::Ready(Ok(()));
        }

        if self.cache_data.is_some() {
            return Poll::Ready(Ok(()));
        }

        let (start, end) = ready!(self.poll_line(cx, reader))?;
        let line = &self.buf[start..end];
        if line[0] == b'.' {
            if line == b".\r\n" {
                self.read_done = true;
                return Poll::Ready(Ok(()));
            }
            self.cache_data = Some((start + 1, end));
            Poll::Ready(Ok(()))
        } else {
            self.cache_data = Some((start, end));
            Poll::Ready(Ok(()))
        }
    }

    fn cache(&self) -> &[u8] {
        if let Some((start, end)) = &self.cache_data {
            &self.buf[*start..*end]
        } else {
            &[]
        }
    }

    fn consume_cache(&mut self, amt: usize) {
        if let Some((start, end)) = self.cache_data.take() {
            if amt < end - start {
                self.cache_data = Some((start + amt, end))
            } else {
                self.start = end
            }
        }
    }
}

pin_project! {
    pub struct TextDataDecodeReader<'a, R> {
        #[pin]
        reader: &'a mut R,
        buf: DataDecodeBuffer,
    }
}

impl<'a, R> TextDataDecodeReader<'a, R> {
    pub fn new(reader: &'a mut R, buf_size: usize) -> Self {
        TextDataDecodeReader {
            reader,
            buf: DataDecodeBuffer::new(buf_size),
        }
    }

    pub fn finished(&self) -> bool {
        self.buf.read_done
    }
}

impl<'a, R> AsyncRead for TextDataDecodeReader<'a, R>
where
    R: AsyncRead + Unpin,
{
    fn poll_read(
        self: Pin<&mut Self>,
        cx: &mut Context<'_>,
        buf: &mut ReadBuf<'_>,
    ) -> Poll<io::Result<()>> {
        let me = self.project();
        me.buf.poll_read(cx, me.reader, buf)
    }
}

impl<'a, R> AsyncBufRead for TextDataDecodeReader<'a, R>
where
    R: AsyncRead + Unpin,
{
    fn poll_fill_buf(self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<io::Result<&[u8]>> {
        let me = self.project();
        ready!(me.buf.poll_get_cache(cx, me.reader))?;

        Poll::Ready(Ok(me.buf.cache()))
    }

    fn consume(mut self: Pin<&mut Self>, amt: usize) {
        self.buf.consume_cache(amt)
    }
}

#[cfg(test)]
mod test {
    use super::*;
    use bytes::Bytes;
    use tokio::io::{AsyncReadExt, BufReader, Result};
    use tokio_util::io::StreamReader;

    #[tokio::test]
    async fn read_single_normal() {
        let body_len: usize = 18;
        let content = b"Line 1\r\n\r\n.Line 2\r\n.\r\n";
        let stream = tokio_stream::iter(vec![Result::Ok(Bytes::from_static(content))]);
        let stream = StreamReader::new(stream);
        let mut buf_stream = BufReader::new(stream);
        let mut body_deocder = TextDataDecodeReader::new(&mut buf_stream, 1024);

        let mut buf = [0u8; 32];
        let len = body_deocder.read(&mut buf).await.unwrap();
        assert_eq!(len, body_len);
        assert_eq!(&buf[0..len], b"Line 1\r\n\r\nLine 2\r\n");
        assert!(body_deocder.finished());
    }

    #[tokio::test]
    async fn read_single_malformed() {
        let body_len: usize = 18;
        let content = b"Line 1\r\n\r\n.Line 2\r\n.\r\n123";
        let stream = tokio_stream::iter(vec![Result::Ok(Bytes::from_static(content))]);
        let stream = StreamReader::new(stream);
        let mut buf_stream = BufReader::new(stream);
        let mut body_deocder = TextDataDecodeReader::new(&mut buf_stream, 1024);

        let mut buf = [0u8; 32];
        let len = body_deocder.read(&mut buf).await.unwrap();
        assert_eq!(len, body_len);
        assert_eq!(&buf[0..len], b"Line 1\r\n\r\nLine 2\r\n");
        assert!(body_deocder.finished());
    }

    #[tokio::test]
    async fn read_multi_normal_read() {
        let body_len: usize = 18;
        let content1 = b"Line 1\r\n\r";
        let content2 = b"\n.";
        let content3 = b"Line 2";
        let content4 = b"\r\n.\r\n";
        let stream = tokio_stream::iter(vec![
            Result::Ok(Bytes::from_static(content1)),
            Result::Ok(Bytes::from_static(content2)),
            Result::Ok(Bytes::from_static(content3)),
            Result::Ok(Bytes::from_static(content4)),
        ]);
        let stream = StreamReader::new(stream);
        let mut buf_stream = BufReader::new(stream);
        let mut body_deocder = TextDataDecodeReader::new(&mut buf_stream, 1024);

        let mut buf = [0u8; 32];
        let len = body_deocder.read(&mut buf).await.unwrap();
        assert_eq!(len, body_len);
        assert_eq!(&buf[0..len], b"Line 1\r\n\r\nLine 2\r\n");
    }

    #[tokio::test]
    async fn read_multi_normal_buf_copy() {
        let body_len: usize = 18;
        let content1 = b"Line 1\r\n\r";
        let content2 = b"\n.";
        let content3 = b"Line 2";
        let content4 = b"\r\n.\r\n";
        let stream = tokio_stream::iter(vec![
            Result::Ok(Bytes::from_static(content1)),
            Result::Ok(Bytes::from_static(content2)),
            Result::Ok(Bytes::from_static(content3)),
            Result::Ok(Bytes::from_static(content4)),
        ]);
        let stream = StreamReader::new(stream);
        let mut buf_stream = BufReader::new(stream);
        let mut body_deocder = TextDataDecodeReader::new(&mut buf_stream, 1024);

        let mut buf = Vec::with_capacity(body_len);
        let len = tokio::io::copy_buf(&mut body_deocder, &mut buf)
            .await
            .unwrap();
        assert_eq!(len, body_len as u64);
        assert_eq!(&buf, b"Line 1\r\n\r\nLine 2\r\n");
        assert!(body_deocder.finished());
    }

    #[tokio::test]
    async fn read_multi_malformed_read() {
        let body_len: usize = 18;
        let content1 = b"Line 1\r\n\r";
        let content2 = b"\n.";
        let content3 = b"Line 2";
        let content4 = b"\r\n.\r\n123";
        let stream = tokio_stream::iter(vec![
            Result::Ok(Bytes::from_static(content1)),
            Result::Ok(Bytes::from_static(content2)),
            Result::Ok(Bytes::from_static(content3)),
            Result::Ok(Bytes::from_static(content4)),
        ]);
        let stream = StreamReader::new(stream);
        let mut buf_stream = BufReader::new(stream);
        let mut body_deocder = TextDataDecodeReader::new(&mut buf_stream, 1024);

        let mut buf = [0u8; 32];
        let len = body_deocder.read(&mut buf).await.unwrap();
        assert_eq!(len, body_len);
        assert_eq!(&buf[0..len], b"Line 1\r\n\r\nLine 2\r\n");
    }

    #[tokio::test]
    async fn read_multi_malformed_buf_copy() {
        let body_len: usize = 18;
        let content1 = b"Line 1\r\n\r";
        let content2 = b"\n.";
        let content3 = b"Line 2";
        let content4 = b"\r\n.\r\n123";
        let stream = tokio_stream::iter(vec![
            Result::Ok(Bytes::from_static(content1)),
            Result::Ok(Bytes::from_static(content2)),
            Result::Ok(Bytes::from_static(content3)),
            Result::Ok(Bytes::from_static(content4)),
        ]);
        let stream = StreamReader::new(stream);
        let mut buf_stream = BufReader::new(stream);
        let mut body_deocder = TextDataDecodeReader::new(&mut buf_stream, 1024);

        let mut buf = Vec::with_capacity(body_len);
        let len = tokio::io::copy_buf(&mut body_deocder, &mut buf)
            .await
            .unwrap();
        assert_eq!(len, body_len as u64);
        assert_eq!(&buf, b"Line 1\r\n\r\nLine 2\r\n");
        assert!(body_deocder.finished());
    }
}
