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

use crate::parse::HttpChunkedLine;

struct ChunkedDecodeReaderInternal {
    body_line_max_size: usize,
    chunk_header: Vec<u8>,
    this_chunk_size: u64,
    left_chunk_size: u64,
    poll_chunk_end_r: bool,
    poll_chunk_end_n: bool,
    poll_chunk_end: bool,
}

impl ChunkedDecodeReaderInternal {
    fn new(body_line_max_size: usize) -> Self {
        ChunkedDecodeReaderInternal {
            body_line_max_size,
            chunk_header: Vec::with_capacity(32),
            this_chunk_size: 0,
            left_chunk_size: 0,
            poll_chunk_end_r: false,
            poll_chunk_end_n: false,
            poll_chunk_end: false,
        }
    }

    fn poll_decode<R>(
        &mut self,
        cx: &mut Context<'_>,
        mut reader: Pin<&mut R>,
        buf: &mut ReadBuf<'_>,
    ) -> Poll<io::Result<()>>
    where
        R: AsyncBufRead + Unpin,
    {
        loop {
            if self.poll_chunk_end {
                if self.this_chunk_size == 0 {
                    return Poll::Ready(Ok(()));
                } else {
                    self.poll_chunk_end = false;
                    self.poll_chunk_end_n = false;
                    self.poll_chunk_end_r = false;
                }
            } else if self.poll_chunk_end_n {
                let r_buf = ready!(reader.as_mut().poll_fill_buf(cx))?;
                if r_buf.is_empty() {
                    return Poll::Ready(Err(io::Error::new(
                        io::ErrorKind::UnexpectedEof,
                        "reader closed while reading chunk line end whitespace",
                    )));
                } else if r_buf[0] == b'\n' {
                    reader.as_mut().consume(1);
                    self.poll_chunk_end = true;
                    continue;
                } else {
                    return Poll::Ready(Err(io::Error::new(
                        io::ErrorKind::InvalidData,
                        "no end whitespace found",
                    )));
                }
            } else if self.poll_chunk_end_r {
                let r_buf = ready!(reader.as_mut().poll_fill_buf(cx))?;
                match r_buf.len() {
                    0 => {
                        return Poll::Ready(Err(io::Error::new(
                            io::ErrorKind::UnexpectedEof,
                            "reader closed while reading chunk line end whitespace",
                        )));
                    }
                    1 => match r_buf[0] {
                        b'\r' => {
                            reader.as_mut().consume(1);
                            self.poll_chunk_end_n = true;
                            continue;
                        }
                        b'\n' => {
                            reader.as_mut().consume(1);
                            self.poll_chunk_end = true;
                            continue;
                        }
                        _ => {
                            return Poll::Ready(Err(io::Error::new(
                                io::ErrorKind::InvalidData,
                                "no end whitespace found",
                            )));
                        }
                    },
                    _ => match r_buf[0] {
                        b'\r' => {
                            if r_buf[1] != b'\n' {
                                return Poll::Ready(Err(io::Error::new(
                                    io::ErrorKind::InvalidData,
                                    "invalid end whitespace pair",
                                )));
                            } else {
                                reader.as_mut().consume(2);
                                self.poll_chunk_end = true;
                                continue;
                            }
                        }
                        b'\n' => {
                            reader.as_mut().consume(1);
                            self.poll_chunk_end = true;
                            continue;
                        }
                        _ => {
                            return Poll::Ready(Err(io::Error::new(
                                io::ErrorKind::InvalidData,
                                "no end whitespace found",
                            )));
                        }
                    },
                }
            } else if self.left_chunk_size > 0 {
                let buf_remaining = buf.remaining();
                if buf_remaining == 0 {
                    return Poll::Ready(Ok(()));
                }

                let to_read = usize::try_from(self.left_chunk_size)
                    .unwrap_or(usize::MAX)
                    .min(buf_remaining);
                let mut new_buf = ReadBuf::new(buf.initialize_unfilled_to(to_read));
                ready!(reader.as_mut().poll_read(cx, &mut new_buf))?;
                let nr = new_buf.filled().len();
                if nr == 0 {
                    return Poll::Ready(Err(io::Error::new(
                        io::ErrorKind::UnexpectedEof,
                        "reader closed while reading chunk data",
                    )));
                }
                buf.advance(nr);
                self.left_chunk_size -= nr as u64;
                if self.left_chunk_size == 0 {
                    self.poll_chunk_end_r = true;
                }
            } else {
                loop {
                    let r_buf = ready!(reader.as_mut().poll_fill_buf(cx))?;
                    if r_buf.is_empty() {
                        return Poll::Ready(Err(io::Error::new(
                            io::ErrorKind::UnexpectedEof,
                            "reader closed while reading chunk line",
                        )));
                    }

                    match memchr::memchr(b'\n', r_buf) {
                        Some(p) => {
                            self.chunk_header.put_slice(&r_buf[0..=p]);
                            reader.as_mut().consume(p + 1);
                            break;
                        }
                        None => {
                            let len = r_buf.len();
                            if self.chunk_header.len() + len > self.body_line_max_size {
                                return Poll::Ready(Err(io::Error::new(
                                    io::ErrorKind::Other,
                                    format!(
                                        "chunk header line too long (> {})",
                                        self.body_line_max_size
                                    ),
                                )));
                            }
                            self.chunk_header.put_slice(r_buf);
                            reader.as_mut().consume(len);
                        }
                    }
                }

                let chunk_line = HttpChunkedLine::parse(&self.chunk_header)
                    .map_err(|e| io::Error::new(io::ErrorKind::InvalidData, e))?;
                self.this_chunk_size = chunk_line.chunk_size;
                self.left_chunk_size = chunk_line.chunk_size;
                if self.left_chunk_size == 0 {
                    self.poll_chunk_end_r = true;
                }
                self.chunk_header.clear();
            }
        }
    }
}

pub struct ChunkedDecodeReader<'a, R> {
    reader: &'a mut R,
    internal: ChunkedDecodeReaderInternal,
}

impl<'a, R> ChunkedDecodeReader<'a, R> {
    pub fn new(reader: &'a mut R, body_line_max_size: usize) -> Self {
        ChunkedDecodeReader {
            reader,
            internal: ChunkedDecodeReaderInternal::new(body_line_max_size),
        }
    }

    pub fn into_reader(self) -> &'a mut R {
        self.reader
    }
}

impl<'a, R> AsyncRead for ChunkedDecodeReader<'a, R>
where
    R: AsyncBufRead + Unpin,
{
    fn poll_read(
        mut self: Pin<&mut Self>,
        cx: &mut Context<'_>,
        buf: &mut ReadBuf<'_>,
    ) -> Poll<io::Result<()>> {
        let me = &mut *self;

        let old_remaining = buf.remaining();
        match me.internal.poll_decode(cx, Pin::new(&mut me.reader), buf) {
            Poll::Pending => {
                if old_remaining > buf.remaining() {
                    Poll::Ready(Ok(()))
                } else {
                    Poll::Pending
                }
            }
            Poll::Ready(r) => Poll::Ready(r),
        }
    }
}
