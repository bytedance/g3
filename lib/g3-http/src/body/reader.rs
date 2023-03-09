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
use std::pin::Pin;
use std::task::{ready, Context, Poll};

use bytes::BufMut;
use tokio::io::{AsyncBufRead, AsyncRead, ReadBuf};

use super::HttpBodyType;
use crate::HttpChunkedLine;

enum NextReadType {
    EndOfFile,
    UntilEnd,
    FixedLength,
    ChunkSize,
    ChunkEnd(u8),
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
        let mut content_length = 0u64;
        let next_read_type = match &body_type {
            HttpBodyType::ContentLength(size) => {
                content_length = *size;
                NextReadType::FixedLength
            }
            HttpBodyType::ChunkedWithoutTrailer | HttpBodyType::ChunkedWithTrailer => {
                NextReadType::ChunkSize
            }
            HttpBodyType::ReadUntilEnd => NextReadType::UntilEnd,
        };
        let mut r = HttpBodyReader {
            stream,
            body_type,
            next_read_type,
            body_line_max_len,
            next_read_size: 0,
            left_total_size: content_length,
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

    pub fn new_chunked_after_preview(
        stream: &'a mut R,
        body_type: HttpBodyType,
        body_line_max_len: usize,
        next_chunk_size: u64,
    ) -> Self {
        let mut r = HttpBodyReader {
            stream,
            body_type,
            next_read_type: NextReadType::FixedLength,
            body_line_max_len,
            next_read_size: 0,
            left_total_size: 0,
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
        assert_eq!(self.next_read_size, 0);
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
                    HttpBodyType::ChunkedWithTrailer | HttpBodyType::ChunkedWithoutTrailer => {
                        // continue to next chunk
                        self.next_read_type = NextReadType::ChunkEnd(b'\r')
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
            self.next_read_type = NextReadType::ChunkEnd(b'\r');
        } else {
            self.next_read_type = NextReadType::FixedLength;
            self.left_total_size = chunk.chunk_size;
            self.update_next_read_size();
        }
        self.chunk_size_line_cache.clear(); // clear only if success
        Ok(())
    }

    fn poll_chunk_end(
        &mut self,
        cx: &mut Context<'_>,
        buf: &mut [u8],
        char: u8,
    ) -> Poll<io::Result<usize>> {
        assert!(b"\r\n".contains(&char));

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
                    self.check_chunk_end_last_char(next_char)?;
                    buf[1] = b'\n';
                    nw = 2;
                } else {
                    reader.as_mut().consume(1);
                    self.next_read_type = NextReadType::ChunkEnd(b'\n');
                }
            }
            b'\n' => {
                reader.as_mut().consume(1);
                self.update_next_read_type_after_chunk_end();
            }
            _ => unreachable!(),
        }

        Poll::Ready(Ok(nw))
    }

    fn check_chunk_end_last_char(&mut self, char: u8) -> io::Result<()> {
        if char != b'\n' {
            return Err(io::Error::new(
                io::ErrorKind::InvalidData,
                "invalid chunk ending last char",
            ));
        }
        self.update_next_read_type_after_chunk_end();
        Ok(())
    }

    fn update_next_read_type_after_chunk_end(&mut self) {
        self.next_read_type = if self.current_chunk_size == 0 {
            match self.body_type {
                HttpBodyType::ChunkedWithoutTrailer => NextReadType::EndOfFile,
                HttpBodyType::ChunkedWithTrailer => NextReadType::Trailer,
                _ => unreachable!(),
            }
        } else {
            NextReadType::ChunkSize
        };
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
                NextReadType::ChunkEnd(char) => {
                    self.as_mut()
                        .poll_chunk_end(cx, buf.initialize_unfilled(), char)
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

impl<'a, R> AsyncRead for HttpBodyReader<'a, R>
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
            HttpBodyType::ChunkedWithoutTrailer | HttpBodyType::ChunkedWithTrailer => {
                self.poll_chunked(cx, buf)
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use bytes::Bytes;
    use tokio::io::{AsyncReadExt, BufReader, Result};
    use tokio_util::io::StreamReader;

    #[tokio::test]
    async fn read_single_to_end() {
        let content = b"test body";
        let stream = tokio_stream::iter(vec![Result::Ok(Bytes::from_static(content))]);
        let stream = StreamReader::new(stream);
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
        let stream = tokio_stream::iter(vec![
            Result::Ok(Bytes::from_static(content1)),
            Result::Ok(Bytes::from_static(content2)),
        ]);
        let stream = StreamReader::new(stream);
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
        let stream = tokio_stream::iter(vec![Result::Ok(Bytes::from_static(content))]);
        let stream = StreamReader::new(stream);
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
        let stream = tokio_stream::iter(vec![
            Result::Ok(Bytes::from_static(content1)),
            Result::Ok(Bytes::from_static(content2)),
        ]);
        let stream = StreamReader::new(stream);
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
    async fn read_single_chunked() {
        let body_len: usize = 24;
        let content = b"5\r\ntest\n\r\n4\r\nbody\r\n0\r\n\r\nXXX";
        let stream = tokio_stream::iter(vec![Result::Ok(Bytes::from_static(content))]);
        let stream = StreamReader::new(stream);
        let mut buf_stream = BufReader::new(stream);
        let mut body_reader =
            HttpBodyReader::new(&mut buf_stream, HttpBodyType::ChunkedWithoutTrailer, 1024);

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
        let stream = tokio_stream::iter(vec![
            Result::Ok(Bytes::from_static(content1)),
            Result::Ok(Bytes::from_static(content2)),
        ]);
        let stream = StreamReader::new(stream);
        let mut buf_stream = BufReader::new(stream);
        let mut body_reader =
            HttpBodyReader::new(&mut buf_stream, HttpBodyType::ChunkedWithoutTrailer, 1024);

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
    async fn read_single_trailer() {
        let body_len: usize = 32;
        let content = b"5\r\ntest\n\r\n4\r\nbody\r\n0\r\n\r\nA: B\r\n\r\nXX";
        let stream = tokio_stream::iter(vec![Result::Ok(Bytes::from_static(content))]);
        let stream = StreamReader::new(stream);
        let mut buf_stream = BufReader::new(stream);
        let mut body_reader =
            HttpBodyReader::new(&mut buf_stream, HttpBodyType::ChunkedWithTrailer, 1024);

        let mut buf = [0u8; 64];
        let len = body_reader.read(&mut buf).await.unwrap();
        assert_eq!(len, body_len);
        assert_eq!(&buf[0..len], &content[0..len]);
        //let len = body_reader.read(&mut buf).await.unwrap();
        //assert_eq!(len, 0);
        assert!(body_reader.finished());
    }
}
