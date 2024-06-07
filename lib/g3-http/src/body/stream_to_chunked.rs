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

use std::future::Future;
use std::io::Write;
use std::pin::Pin;
use std::task::{ready, Context, Poll};

use tokio::io::{AsyncBufRead, AsyncWrite};

use g3_io_ext::LimitedCopyError;

struct ChunkedNoTrailerEncodeTransferInternal {
    yield_size: usize,
    no_trailer: bool,
    this_chunk_size: usize,
    left_chunk_size: usize,
    static_header: Vec<u8>,
    static_offset: usize,
    total_write: u64,
    read_finished: bool,
    active: bool,
}

impl ChunkedNoTrailerEncodeTransferInternal {
    fn new(yield_size: usize, no_trailer: bool) -> Self {
        ChunkedNoTrailerEncodeTransferInternal {
            yield_size,
            no_trailer,
            this_chunk_size: 0,
            left_chunk_size: 0,
            static_header: Vec::with_capacity(16),
            static_offset: 0,
            total_write: 0,
            read_finished: false,
            active: false,
        }
    }

    fn poll_encode<R, W>(
        &mut self,
        cx: &mut Context<'_>,
        mut reader: Pin<&mut R>,
        mut writer: Pin<&mut W>,
    ) -> Poll<Result<u64, LimitedCopyError>>
    where
        R: AsyncBufRead,
        W: AsyncWrite,
    {
        let mut copy_this_round = 0usize;
        loop {
            if self.this_chunk_size == 0 && !self.read_finished {
                let data = ready!(reader.as_mut().poll_fill_buf(cx))
                    .map_err(LimitedCopyError::ReadFailed)?;
                self.active = true;
                self.static_header.clear();
                let chunk_size = data.len();
                if chunk_size == 0 {
                    self.read_finished = true;
                    if self.total_write == 0 {
                        if self.no_trailer {
                            let _ = write!(&mut self.static_header, "0\r\n\r\n");
                        } else {
                            let _ = write!(&mut self.static_header, "0\r\n");
                        }
                    } else if self.no_trailer {
                        let _ = write!(&mut self.static_header, "\r\n0\r\n\r\n");
                    } else {
                        let _ = write!(&mut self.static_header, "\r\n0\r\n");
                    }
                } else if self.total_write == 0 {
                    let _ = write!(&mut self.static_header, "{chunk_size:x}\r\n");
                } else {
                    let _ = write!(&mut self.static_header, "\r\n{chunk_size:x}\r\n");
                }
                self.static_offset = 0;
                self.this_chunk_size = chunk_size;
                self.left_chunk_size = chunk_size;
            }

            while self.static_offset < self.static_header.len() {
                let nw = ready!(writer
                    .as_mut()
                    .poll_write(cx, &self.static_header[self.static_offset..]))
                .map_err(LimitedCopyError::WriteFailed)?;
                self.active = true;
                self.static_offset += nw;
                self.total_write += nw as u64;
            }
            if self.read_finished {
                return Poll::Ready(Ok(self.total_write));
            }

            while self.left_chunk_size > 0 {
                let data = ready!(reader
                    .as_mut()
                    .poll_fill_buf(cx)
                    .map_err(LimitedCopyError::ReadFailed))?;
                debug_assert!(self.left_chunk_size <= data.len());
                let nw = ready!(writer
                    .as_mut()
                    .poll_write(cx, &data[..self.left_chunk_size]))
                .map_err(LimitedCopyError::WriteFailed)?;
                reader.as_mut().consume(nw);
                copy_this_round += nw;
                self.active = true;
                self.left_chunk_size -= nw;
                self.total_write += nw as u64;
            }
            self.this_chunk_size = 0;

            if copy_this_round >= self.yield_size {
                cx.waker().wake_by_ref();
                return Poll::Pending;
            }
        }
    }

    #[inline]
    fn finished(&self) -> bool {
        self.read_finished && self.static_offset >= self.static_header.len()
    }

    #[inline]
    fn is_idle(&self) -> bool {
        !self.active
    }

    #[inline]
    fn is_active(&self) -> bool {
        self.active
    }

    fn reset_active(&mut self) {
        self.active = false;
    }

    fn no_cached_data(&self) -> bool {
        self.static_offset >= self.static_header.len() && self.left_chunk_size == 0
    }
}

pub struct StreamToChunkedTransfer<'a, R, W> {
    reader: &'a mut R,
    writer: &'a mut W,
    internal: ChunkedNoTrailerEncodeTransferInternal,
}

impl<'a, R, W> StreamToChunkedTransfer<'a, R, W> {
    pub fn new(reader: &'a mut R, writer: &'a mut W, yield_size: usize, no_trailer: bool) -> Self {
        StreamToChunkedTransfer {
            reader,
            writer,
            internal: ChunkedNoTrailerEncodeTransferInternal::new(yield_size, no_trailer),
        }
    }

    pub fn new_with_no_trailer(reader: &'a mut R, writer: &'a mut W, yield_size: usize) -> Self {
        Self::new(reader, writer, yield_size, true)
    }

    pub fn new_with_pending_trailer(
        reader: &'a mut R,
        writer: &'a mut W,
        yield_size: usize,
    ) -> Self {
        Self::new(reader, writer, yield_size, false)
    }

    pub fn finished(&self) -> bool {
        self.internal.finished()
    }

    pub fn is_idle(&self) -> bool {
        self.internal.is_idle()
    }

    pub fn is_active(&self) -> bool {
        self.internal.is_active()
    }

    pub fn reset_active(&mut self) {
        self.internal.reset_active()
    }

    pub fn no_cached_data(&self) -> bool {
        self.internal.no_cached_data()
    }
}

impl<'a, R, W> Future for StreamToChunkedTransfer<'a, R, W>
where
    R: AsyncBufRead + Unpin,
    W: AsyncWrite + Unpin,
{
    type Output = Result<u64, LimitedCopyError>;

    fn poll(mut self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Self::Output> {
        let me = &mut *self;

        me.internal
            .poll_encode(cx, Pin::new(&mut me.reader), Pin::new(&mut me.writer))
    }
}

#[cfg(test)]
mod test {
    use super::*;
    use bytes::Bytes;
    use tokio::io::{BufReader, Result};
    use tokio_util::io::StreamReader;

    #[tokio::test]
    async fn encode_two_no_trailer() {
        let body_len: usize = 24;
        let data1 = b"test\n";
        let data2 = b"body";
        let stream = tokio_stream::iter(vec![
            Result::Ok(Bytes::from_static(data1)),
            Result::Ok(Bytes::from_static(data2)),
        ]);
        let stream = StreamReader::new(stream);
        let mut buf_stream = BufReader::new(stream);

        let mut write_buf = Vec::with_capacity(body_len);

        let mut chunked_encoder =
            StreamToChunkedTransfer::new_with_no_trailer(&mut buf_stream, &mut write_buf, 1024);

        let nw = (&mut chunked_encoder).await.unwrap();
        assert_eq!(nw, body_len as u64);
        assert!(chunked_encoder.finished());

        assert_eq!(&write_buf, b"5\r\ntest\n\r\n4\r\nbody\r\n0\r\n\r\n");
    }

    #[tokio::test]
    async fn encode_two_pending_trailer() {
        let body_len: usize = 22;
        let data1 = b"test\n";
        let data2 = b"body";
        let stream = tokio_stream::iter(vec![
            Result::Ok(Bytes::from_static(data1)),
            Result::Ok(Bytes::from_static(data2)),
        ]);
        let stream = StreamReader::new(stream);
        let mut buf_stream = BufReader::new(stream);

        let mut write_buf = Vec::with_capacity(body_len);

        let mut chunked_encoder = StreamToChunkedTransfer::new_with_pending_trailer(
            &mut buf_stream,
            &mut write_buf,
            1024,
        );

        let nw = (&mut chunked_encoder).await.unwrap();
        assert_eq!(nw, body_len as u64);
        assert!(chunked_encoder.finished());

        assert_eq!(&write_buf, b"5\r\ntest\n\r\n4\r\nbody\r\n0\r\n");
    }

    #[tokio::test]
    async fn encode_empty_no_trailer() {
        let body_len: usize = 5;
        let data1 = b"";
        let stream = tokio_stream::iter(vec![Result::Ok(Bytes::from_static(data1))]);
        let stream = StreamReader::new(stream);
        let mut buf_stream = BufReader::new(stream);

        let mut write_buf = Vec::with_capacity(body_len);

        let mut chunked_encoder =
            StreamToChunkedTransfer::new_with_no_trailer(&mut buf_stream, &mut write_buf, 1024);

        let nw = (&mut chunked_encoder).await.unwrap();
        assert_eq!(nw, body_len as u64);
        assert!(chunked_encoder.finished());

        assert_eq!(&write_buf, b"0\r\n\r\n");
    }

    #[tokio::test]
    async fn encode_empty_pending_trailer() {
        let body_len: usize = 3;
        let data1 = b"";
        let stream = tokio_stream::iter(vec![Result::Ok(Bytes::from_static(data1))]);
        let stream = StreamReader::new(stream);
        let mut buf_stream = BufReader::new(stream);

        let mut write_buf = Vec::with_capacity(body_len);

        let mut chunked_encoder = StreamToChunkedTransfer::new_with_pending_trailer(
            &mut buf_stream,
            &mut write_buf,
            1024,
        );

        let nw = (&mut chunked_encoder).await.unwrap();
        assert_eq!(nw, body_len as u64);
        assert!(chunked_encoder.finished());

        assert_eq!(&write_buf, b"0\r\n");
    }
}
