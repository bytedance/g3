/*
 * SPDX-License-Identifier: Apache-2.0
 * Copyright 2023-2025 ByteDance and/or its affiliates.
 */

use std::io;
use std::pin::Pin;
use std::task::{Context, Poll, ready};

use bytes::BufMut;
use tokio::io::{AsyncBufRead, AsyncRead, ReadBuf};

use super::HttpBodyType;
use crate::HttpChunkedLine;

enum NextReadType {
    EndOfFile,
    UntilEnd,
    FixedLength,
    ChunkSize,
    ChunkDataEnd(u8),
    Trailer,
}

pub struct HttpBodyReader<'a, R> {
    stream: &'a mut R,
    body_type: HttpBodyType,
    next_read_type: NextReadType,
    body_line_max_len: usize,

    next_read_size: usize,
    left_total_size: u64,

    chunk_size_line_cache: Vec<u8>,

    trailer_line_length: usize,
    trailer_last_char: u8,

    finished: bool,
    read_content_length: u64,
    current_chunk_size: u64,
}

impl<'a, R> HttpBodyReader<'a, R>
where
    R: AsyncBufRead + Unpin,
{
    const DEFAULT_LINE_SIZE: usize = 64;

    pub fn new(stream: &'a mut R, body_type: HttpBodyType, body_line_max_len: usize) -> Self {
        match body_type {
            HttpBodyType::ReadUntilEnd => HttpBodyReader::new_read_until_end(stream),
            HttpBodyType::ContentLength(len) => HttpBodyReader::new_fixed_length(stream, len),
            HttpBodyType::Chunked => HttpBodyReader::new_chunked(stream, body_line_max_len),
        }
    }

    pub fn new_read_until_end(stream: &'a mut R) -> Self {
        let mut r = HttpBodyReader {
            stream,
            body_type: HttpBodyType::ReadUntilEnd,
            next_read_type: NextReadType::UntilEnd,
            body_line_max_len: 1024,
            next_read_size: 0,
            left_total_size: 0,
            chunk_size_line_cache: Vec::new(),
            trailer_line_length: 0,
            trailer_last_char: 0,
            finished: false,
            read_content_length: 0,
            current_chunk_size: 0,
        };
        r.update_next_read_size();
        r
    }

    pub fn new_fixed_length(stream: &'a mut R, content_length: u64) -> Self {
        let mut r = HttpBodyReader {
            stream,
            body_type: HttpBodyType::ContentLength(content_length),
            next_read_type: NextReadType::FixedLength,
            body_line_max_len: 1024,
            next_read_size: 0,
            left_total_size: content_length,
            chunk_size_line_cache: Vec::new(),
            trailer_line_length: 0,
            trailer_last_char: 0,
            finished: false,
            read_content_length: 0,
            current_chunk_size: 0,
        };
        r.update_next_read_size();
        r
    }

    pub fn new_chunked(stream: &'a mut R, body_line_max_len: usize) -> Self {
        let mut r = HttpBodyReader {
            stream,
            body_type: HttpBodyType::Chunked,
            next_read_type: NextReadType::ChunkSize,
            body_line_max_len,
            next_read_size: 0,
            left_total_size: 0,
            chunk_size_line_cache: Vec::<u8>::with_capacity(Self::DEFAULT_LINE_SIZE),
            trailer_line_length: 0,
            trailer_last_char: 0,
            finished: false,
            read_content_length: 0,
            current_chunk_size: 0,
        };
        r.update_next_read_size();
        r
    }

    pub fn new_trailer(stream: &'a mut R, body_line_max_len: usize) -> Self {
        HttpBodyReader {
            stream,
            body_type: HttpBodyType::Chunked,
            next_read_type: NextReadType::Trailer,
            body_line_max_len,
            next_read_size: 0,
            left_total_size: 0,
            chunk_size_line_cache: Vec::<u8>::with_capacity(Self::DEFAULT_LINE_SIZE),
            trailer_line_length: 0,
            trailer_last_char: 0,
            finished: false,
            read_content_length: 0,
            current_chunk_size: 0,
        }
    }

    pub fn new_chunked_after_preview(
        stream: &'a mut R,
        body_line_max_len: usize,
        next_chunk_size: u64,
    ) -> Self {
        let mut r = HttpBodyReader {
            stream,
            body_type: HttpBodyType::Chunked,
            next_read_type: NextReadType::FixedLength,
            body_line_max_len,
            next_read_size: 0,
            left_total_size: next_chunk_size,
            chunk_size_line_cache: Vec::<u8>::with_capacity(Self::DEFAULT_LINE_SIZE),
            trailer_line_length: 0,
            trailer_last_char: 0,
            finished: false,
            read_content_length: 0,
            current_chunk_size: next_chunk_size,
        };
        r.update_next_read_size();
        r
    }

    pub fn finished(&self) -> bool {
        self.finished
    }

    fn update_next_read_size(&mut self) {
        const MAX_USIZE: usize = usize::MAX;
        debug_assert_eq!(self.next_read_size, 0);
        if self.left_total_size > MAX_USIZE as u64 {
            self.next_read_size = MAX_USIZE;
            self.left_total_size -= MAX_USIZE as u64;
        } else if self.left_total_size > 0 {
            self.next_read_size = self.left_total_size as usize;
            self.left_total_size = 0;
        }
    }

    fn poll_eof(&mut self, cx: &mut Context<'_>, buf: &mut ReadBuf<'_>) -> Poll<io::Result<()>> {
        let old_remaining = buf.remaining();
        ready!(Pin::new(&mut *self.stream).poll_read(cx, buf))?;
        let nr = old_remaining - buf.remaining();
        if nr == 0 {
            // io closed, which indicate the end of body
            self.finished = true;
        } else {
            self.read_content_length += nr as u64;
        }
        Poll::Ready(Ok(()))
    }

    fn poll_fixed(&mut self, cx: &mut Context<'_>, buf: &mut ReadBuf<'_>) -> Poll<io::Result<()>> {
        let buf_len = std::cmp::min(buf.remaining(), self.next_read_size);
        let mut limited_buf = ReadBuf::new(buf.initialize_unfilled_to(buf_len));
        ready!(Pin::new(&mut *self.stream).poll_read(cx, &mut limited_buf))?;
        let nr = limited_buf.filled().len();
        if nr == 0 {
            // io closed unexpectedly
            return Poll::Ready(Err(io::Error::new(
                io::ErrorKind::UnexpectedEof,
                "reader closed while reading fixed length body",
            )));
        }
        buf.advance(nr);

        self.read_content_length += nr as u64;
        self.next_read_size -= nr;

        if self.next_read_size == 0 {
            self.update_next_read_size();
            if self.next_read_size == 0 {
                // all data in this chunk/slice has been read out
                match self.body_type {
                    HttpBodyType::ContentLength(_) => self.next_read_type = NextReadType::EndOfFile,
                    HttpBodyType::Chunked => {
                        // read chunk data end
                        self.next_read_type = NextReadType::ChunkDataEnd(b'\r')
                    }
                    _ => unreachable!(),
                }
            }
        }

        Poll::Ready(Ok(()))
    }

    fn poll_chunk_size(
        &mut self,
        cx: &mut Context<'_>,
        mut buf: &mut [u8],
    ) -> Poll<io::Result<usize>> {
        let mut reader = Pin::new(&mut self.stream);
        let cache = ready!(reader.as_mut().poll_fill_buf(cx))?;
        if cache.is_empty() {
            return Poll::Ready(Err(io::Error::new(
                io::ErrorKind::UnexpectedEof,
                "reader closed while reading chunk line",
            )));
        }

        // do not parse more data than really needed
        let max_len = buf.len();
        let cache = if max_len < cache.len() {
            &cache[0..max_len]
        } else {
            cache
        };

        if matches!(self.chunk_size_line_cache.last(), Some(b'\r')) {
            let next_char = cache[0];
            reader.as_mut().consume(1);
            self.check_chunk_size_last_char(next_char)?;
            buf.put_u8(next_char);
            Poll::Ready(Ok(1))
        } else if let Some(offset) = memchr::memchr(b'\r', cache) {
            let nw = offset + 1;
            // check line size
            if self.chunk_size_line_cache.len() + nw >= self.body_line_max_len {
                return Poll::Ready(Err(io::Error::new(
                    io::ErrorKind::InvalidData,
                    "chunk size line too long",
                )));
            }
            let to_copy = &cache[0..nw];
            buf.put_slice(to_copy);
            self.chunk_size_line_cache.extend_from_slice(to_copy);
            if cache.len() > nw {
                // a small trick to speed up
                let next_char = cache[nw];
                reader.as_mut().consume(nw + 1);
                self.check_chunk_size_last_char(next_char)?;
                buf.put_u8(next_char);
                Poll::Ready(Ok(nw + 1))
            } else {
                reader.as_mut().consume(nw);
                Poll::Ready(Ok(nw))
            }
        } else {
            let nw = cache.len();
            // check line size
            if self.chunk_size_line_cache.len() + nw >= self.body_line_max_len {
                return Poll::Ready(Err(io::Error::new(
                    io::ErrorKind::InvalidData,
                    "chunk size line too long",
                )));
            }
            buf.put_slice(cache);
            self.chunk_size_line_cache.extend_from_slice(cache);
            reader.as_mut().consume(nw);
            Poll::Ready(Ok(nw))
        }
    }

    fn check_chunk_size_last_char(&mut self, char: u8) -> io::Result<()> {
        if char != b'\n' {
            return Err(io::Error::new(
                io::ErrorKind::InvalidData,
                "invalid chunk size line ending",
            ));
        }
        self.chunk_size_line_cache.put_u8(b'\n');

        self.parse_chunk_size_and_update_next_read_type()?;
        Ok(())
    }

    fn parse_chunk_size_and_update_next_read_type(&mut self) -> io::Result<()> {
        let chunk = HttpChunkedLine::parse(self.chunk_size_line_cache.as_slice())
            .map_err(|e| io::Error::new(io::ErrorKind::InvalidData, e))?;
        self.current_chunk_size = chunk.chunk_size;
        if chunk.chunk_size == 0 {
            self.next_read_type = NextReadType::Trailer;
        } else {
            self.next_read_type = NextReadType::FixedLength;
            self.left_total_size = chunk.chunk_size;
            self.update_next_read_size();
        }
        self.chunk_size_line_cache.clear(); // clear only if success
        Ok(())
    }

    fn poll_chunk_data_end(
        &mut self,
        cx: &mut Context<'_>,
        buf: &mut [u8],
        char: u8,
    ) -> Poll<io::Result<usize>> {
        debug_assert!(b"\r\n".contains(&char));

        let mut reader = Pin::new(&mut *self.stream);
        let cache = ready!(reader.as_mut().poll_fill_buf(cx))?;
        if cache.is_empty() {
            return Poll::Ready(Err(io::Error::new(
                io::ErrorKind::UnexpectedEof,
                "reader closed while reading chunk line end whitespace",
            )));
        }

        // do not parse more data than really needed
        let max_len = buf.len();
        let cache = if max_len < cache.len() {
            &cache[0..max_len]
        } else {
            cache
        };

        if cache[0] != char {
            return Poll::Ready(Err(io::Error::new(
                io::ErrorKind::InvalidData,
                "invalid chunk ending first char",
            )));
        }

        buf[0] = char;
        let mut nw: usize = 1;
        match char {
            b'\r' => {
                if cache.len() > 1 {
                    // a small trick to speed up
                    let next_char = cache[1];
                    reader.as_mut().consume(2);
                    self.check_chunk_data_end_last_char(next_char)?;
                    buf[1] = b'\n';
                    nw = 2;
                } else {
                    reader.as_mut().consume(1);
                    self.next_read_type = NextReadType::ChunkDataEnd(b'\n');
                }
            }
            b'\n' => {
                reader.as_mut().consume(1);
                self.update_next_read_type_after_chunk_data_end();
            }
            _ => unreachable!(),
        }

        Poll::Ready(Ok(nw))
    }

    fn check_chunk_data_end_last_char(&mut self, char: u8) -> io::Result<()> {
        if char != b'\n' {
            return Err(io::Error::new(
                io::ErrorKind::InvalidData,
                "invalid chunk ending last char",
            ));
        }
        self.update_next_read_type_after_chunk_data_end();
        Ok(())
    }

    fn update_next_read_type_after_chunk_data_end(&mut self) {
        self.next_read_type = NextReadType::ChunkSize;
    }

    fn poll_trailer(
        &mut self,
        cx: &mut Context<'_>,
        mut buf: &mut [u8],
    ) -> Poll<io::Result<usize>> {
        let mut reader = Pin::new(&mut *self.stream);
        let cache = ready!(reader.as_mut().poll_fill_buf(cx))?;
        if cache.is_empty() {
            return Poll::Ready(Err(io::Error::new(
                io::ErrorKind::UnexpectedEof,
                "reader closed while reading trailer",
            )));
        }

        // do not parse more data than really needed
        let max_len = buf.len();
        let cache = if max_len < cache.len() {
            &cache[0..max_len]
        } else {
            cache
        };

        if self.trailer_line_length != 0 && self.trailer_last_char == b'\r' {
            let next_char = cache[0];
            reader.as_mut().consume(1);
            self.check_trailer_header_last_char(next_char)?;
            buf.put_u8(next_char);
            Poll::Ready(Ok(1))
        } else if let Some(offset) = memchr::memchr(b'\r', cache) {
            let nw = offset + 1;
            // check line size
            if self.trailer_line_length + nw >= self.body_line_max_len {
                return Poll::Ready(Err(io::Error::new(
                    io::ErrorKind::InvalidData,
                    "trailer line too long",
                )));
            }
            buf.put_slice(&cache[0..nw]);
            self.trailer_last_char = b'\r';
            self.trailer_line_length += offset;
            if cache.len() > nw {
                // a small trick to speed up
                let next_char = cache[nw];
                reader.as_mut().consume(nw + 1);
                self.check_trailer_header_last_char(next_char)?;
                buf.put_u8(next_char);
                Poll::Ready(Ok(nw + 1))
            } else {
                reader.as_mut().consume(nw);
                Poll::Ready(Ok(nw))
            }
        } else {
            let nw = cache.len();
            // check line size
            if self.trailer_line_length + nw >= self.body_line_max_len {
                return Poll::Ready(Err(io::Error::new(
                    io::ErrorKind::InvalidData,
                    "trailer line too long",
                )));
            }
            buf.put_slice(cache);
            self.trailer_last_char = cache[nw - 1];
            self.trailer_line_length += nw;
            reader.as_mut().consume(nw);
            Poll::Ready(Ok(nw))
        }
    }

    fn check_trailer_header_last_char(&mut self, char: u8) -> io::Result<()> {
        if char != b'\n' {
            return Err(io::Error::new(
                io::ErrorKind::InvalidData,
                "invalid trailer line ending",
            ));
        }
        self.update_next_read_type_after_trailer_header();
        Ok(())
    }

    fn update_next_read_type_after_trailer_header(&mut self) {
        self.next_read_type = if self.trailer_line_length == 0 {
            NextReadType::EndOfFile
        } else {
            // clear only when needed
            self.trailer_last_char = 0;
            self.trailer_line_length = 0;
            NextReadType::Trailer
        };
    }

    fn poll_chunked(
        mut self: Pin<&mut Self>,
        cx: &mut Context<'_>,
        buf: &mut ReadBuf<'_>,
    ) -> Poll<io::Result<()>> {
        let expect_len = buf.remaining();
        let mut offset: usize = 0;
        loop {
            if offset >= expect_len {
                return Poll::Ready(Ok(()));
            }

            let ret = match self.next_read_type {
                NextReadType::EndOfFile => {
                    self.finished = true;
                    return Poll::Ready(Ok(()));
                }
                NextReadType::ChunkSize => {
                    self.as_mut().poll_chunk_size(cx, buf.initialize_unfilled())
                }
                NextReadType::FixedLength => {
                    let mut inner_buf = ReadBuf::new(buf.initialize_unfilled());
                    // use a wrapper buf to get filled size and keep the code clean
                    self.as_mut()
                        .poll_fixed(cx, &mut inner_buf)
                        .map_ok(|_| inner_buf.filled().len())
                }
                NextReadType::ChunkDataEnd(char) => {
                    self.as_mut()
                        .poll_chunk_data_end(cx, buf.initialize_unfilled(), char)
                }
                NextReadType::Trailer => self.as_mut().poll_trailer(cx, buf.initialize_unfilled()),
                _ => unreachable!(),
            };
            match ret {
                Poll::Pending => {
                    return if offset != 0 {
                        Poll::Ready(Ok(()))
                    } else {
                        Poll::Pending
                    };
                }
                Poll::Ready(Ok(0)) => return Poll::Ready(Ok(())),
                Poll::Ready(Ok(nr)) => {
                    buf.advance(nr);
                    offset += nr;
                }
                Poll::Ready(Err(e)) => return Poll::Ready(Err(e)),
            }
        }
    }
}

impl<R> AsyncRead for HttpBodyReader<'_, R>
where
    R: AsyncBufRead + Unpin,
{
    fn poll_read(
        mut self: Pin<&mut Self>,
        cx: &mut Context<'_>,
        buf: &mut ReadBuf<'_>,
    ) -> Poll<io::Result<()>> {
        match self.body_type {
            HttpBodyType::ReadUntilEnd => self.poll_eof(cx, buf),
            HttpBodyType::ContentLength(_) => match self.next_read_type {
                NextReadType::EndOfFile => {
                    self.finished = true;
                    Poll::Ready(Ok(()))
                }
                NextReadType::FixedLength => self.poll_fixed(cx, buf),
                _ => unreachable!(),
            },
            HttpBodyType::Chunked => self.poll_chunked(cx, buf),
        }
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
        let mut body_reader =
            HttpBodyReader::new(&mut buf_stream, HttpBodyType::ReadUntilEnd, 1024);

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
        let mut body_reader =
            HttpBodyReader::new(&mut buf_stream, HttpBodyType::ReadUntilEnd, 1024);

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
        let mut body_reader = HttpBodyReader::new(
            &mut buf_stream,
            HttpBodyType::ContentLength(body_len as u64),
            1024,
        );

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
        let mut body_reader = HttpBodyReader::new(
            &mut buf_stream,
            HttpBodyType::ContentLength(body_len as u64),
            1024,
        );

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
        let body_len: usize = 5;
        let content = b"0\r\n\r\n";
        let stream = tokio_test::io::Builder::new().read(content).build();
        let mut buf_stream = BufReader::new(stream);
        let mut body_reader = HttpBodyReader::new_chunked(&mut buf_stream, 1024);

        let mut buf = [0u8; 32];
        let len = body_reader.read(&mut buf).await.unwrap();
        assert_eq!(len, body_len);
        assert_eq!(&buf[0..len], &content[0..len]);
        assert!(body_reader.finished());
    }

    #[tokio::test]
    async fn read_single_chunked() {
        let body_len: usize = 24;
        let content = b"5\r\ntest\n\r\n4\r\nbody\r\n0\r\n\r\nXXX";
        let stream = tokio_test::io::Builder::new().read(content).build();
        let mut buf_stream = BufReader::new(stream);
        let mut body_reader = HttpBodyReader::new(&mut buf_stream, HttpBodyType::Chunked, 1024);

        let mut buf = [0u8; 32];
        let len = body_reader.read(&mut buf).await.unwrap();
        assert_eq!(len, body_len);
        assert_eq!(&buf[0..len], &content[0..len]);
        assert!(body_reader.finished());
    }

    #[tokio::test]
    async fn read_split_chunked() {
        let body_len: usize = 24;
        let content1 = b"5\r\ntest\n\r\n4\r";
        let content2 = b"\nbody\r\n0\r\n\r\nXXX";
        let stream = tokio_test::io::Builder::new()
            .read(content1)
            .read(content2)
            .build();
        let mut buf_stream = BufReader::new(stream);
        let mut body_reader = HttpBodyReader::new(&mut buf_stream, HttpBodyType::Chunked, 1024);

        let mut buf = [0u8; 32];
        let len = body_reader.read(&mut buf).await.unwrap();
        assert_eq!(len, body_len);
        assert_eq!(&buf[0..content1.len()], content1);
        assert_eq!(
            &buf[content1.len()..body_len],
            &content2[0..body_len - content1.len()]
        );
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
        let mut body_reader = HttpBodyReader::new(&mut buf_stream, HttpBodyType::Chunked, 1024);

        let mut buf = [0u8; 32];
        let len = body_reader.read(&mut buf).await.unwrap();
        assert_eq!(len, buf.len());
        assert_eq!(
            buf.as_slice(),
            b"5\r\ntest\n\r\n4\r\nbody\r\n20\r\naabbbbbbb"
        );
        assert!(!body_reader.finished());

        let len = body_reader.read(&mut buf).await.unwrap();
        assert_eq!(len, 30);
        assert_eq!(&buf[..len], b"bbbccccccccccdddddddddd\r\n0\r\n\r\n");
        assert!(body_reader.finished());
    }

    #[tokio::test]
    async fn read_single_trailer() {
        let body_len: usize = 30;
        let content = b"5\r\ntest\n\r\n4\r\nbody\r\n0\r\nA: B\r\n\r\nXX";
        let stream = tokio_test::io::Builder::new().read(content).build();
        let mut buf_stream = BufReader::new(stream);
        let mut body_reader = HttpBodyReader::new(&mut buf_stream, HttpBodyType::Chunked, 1024);

        let mut buf = [0u8; 64];
        let len = body_reader.read(&mut buf).await.unwrap();
        assert_eq!(len, body_len);
        assert_eq!(&buf[0..len], &content[0..len]);
        //let len = body_reader.read(&mut buf).await.unwrap();
        //assert_eq!(len, 0);
        assert!(body_reader.finished());
    }
}
