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

use std::future::{poll_fn, Future};
use std::io;
use std::pin::Pin;
use std::task::{ready, Context, Poll};

use tokio::io::{AsyncRead, AsyncWrite, AsyncWriteExt, ReadBuf};

use g3_io_ext::{LimitedCopyConfig, LimitedCopyError};

#[derive(Debug)]
struct EncodeCopyBuffer {
    read_done: bool,
    buf: Box<[u8]>,
    yield_size: usize,
    r_off: usize,
    w_off: usize,
    total: u64,
    need_flush: bool,
    active: bool,

    line_end: usize,
    check_line: bool,
    write_dot: bool,
    read_end_ok: bool,
    write_append_line: bool,
}

impl EncodeCopyBuffer {
    fn new(config: LimitedCopyConfig) -> Self {
        EncodeCopyBuffer {
            read_done: false,
            buf: vec![0; config.buffer_size()].into_boxed_slice(),
            yield_size: config.yield_size(),
            r_off: 0,
            w_off: 0,
            total: 0,
            need_flush: false,
            active: false,
            line_end: 0,
            check_line: true,
            write_dot: false,
            read_end_ok: true, // consider true for empty data
            write_append_line: false,
        }
    }

    fn poll_fill_buf<R>(
        &mut self,
        cx: &mut Context<'_>,
        reader: Pin<&mut R>,
    ) -> Poll<io::Result<()>>
    where
        R: AsyncRead + ?Sized,
    {
        const MIN_BUF_SIZE: usize = 16;

        if self.r_off + MIN_BUF_SIZE >= self.buf.len() {
            // only read if we have enough capacity
            return Poll::Ready(Ok(()));
        }

        let mut buf = ReadBuf::new(&mut self.buf);
        buf.set_filled(self.r_off);

        let res = reader.poll_read(cx, &mut buf);
        if let Poll::Ready(Ok(_)) = res {
            let filled_len = buf.filled().len();
            if self.r_off == filled_len {
                self.read_done = true;
            } else {
                self.r_off = filled_len;
                self.read_end_ok = buf.filled()[filled_len - 1] == b'\n';
                self.active = true;
            }
        }
        res
    }

    fn poll_write_dot<W>(
        &mut self,
        cx: &mut Context<'_>,
        writer: Pin<&mut W>,
    ) -> Poll<Result<usize, LimitedCopyError>>
    where
        W: AsyncWrite + ?Sized,
    {
        match writer.poll_write(cx, b".") {
            Poll::Pending => Poll::Pending,
            Poll::Ready(Err(e)) => Poll::Ready(Err(LimitedCopyError::WriteFailed(e))),
            Poll::Ready(Ok(0)) => Poll::Ready(Err(LimitedCopyError::WriteFailed(io::Error::new(
                io::ErrorKind::WriteZero,
                "write zero byte into writer",
            )))),
            Poll::Ready(Ok(n)) => {
                self.total += n as u64;
                self.need_flush = true;
                self.active = true;
                Poll::Ready(Ok(n))
            }
        }
    }

    fn poll_write_line<W>(
        &mut self,
        cx: &mut Context<'_>,
        writer: Pin<&mut W>,
    ) -> Poll<Result<usize, LimitedCopyError>>
    where
        W: AsyncWrite + ?Sized,
    {
        match writer.poll_write(cx, &self.buf[self.w_off..self.line_end]) {
            Poll::Pending => Poll::Pending,
            Poll::Ready(Err(e)) => Poll::Ready(Err(LimitedCopyError::WriteFailed(e))),
            Poll::Ready(Ok(0)) => Poll::Ready(Err(LimitedCopyError::WriteFailed(io::Error::new(
                io::ErrorKind::WriteZero,
                "write zero byte into writer",
            )))),
            Poll::Ready(Ok(n)) => {
                self.w_off += n;
                self.total += n as u64;
                self.need_flush = true;
                self.active = true;
                Poll::Ready(Ok(n))
            }
        }
    }

    fn poll_write_buf<W>(
        &mut self,
        cx: &mut Context<'_>,
        mut writer: Pin<&mut W>,
    ) -> Poll<Result<usize, LimitedCopyError>>
    where
        W: AsyncWrite + ?Sized,
    {
        let mut nw = 0;
        // If our buffer has some data, let's write it out!
        while self.w_off < self.r_off {
            if self.write_dot {
                let i = ready!(self.poll_write_dot(cx, writer.as_mut()))?;
                nw += i;
                self.write_dot = false;
            }

            let left = &self.buf[self.w_off..self.r_off];
            if self.line_end <= self.w_off {
                self.line_end = match memchr::memchr(b'\n', left) {
                    Some(p) => self.w_off + p + 1,
                    None => self.r_off,
                };
            }

            if self.check_line {
                self.check_line = false;
                if left[0] == b'.' {
                    self.write_dot = true;
                    continue;
                }
            }

            // return if write blocked. no need to try flush
            let i = ready!(self.poll_write_line(cx, writer.as_mut()))?;
            nw += i;
            if self.w_off >= self.line_end {
                self.check_line = true;
            }
        }
        if self.line_end == self.r_off {
            self.line_end = 0;
        }

        Poll::Ready(Ok(nw))
    }

    fn poll_copy<R, W>(
        &mut self,
        cx: &mut Context<'_>,
        mut reader: Pin<&mut R>,
        mut writer: Pin<&mut W>,
    ) -> Poll<Result<u64, LimitedCopyError>>
    where
        R: AsyncRead + ?Sized,
        W: AsyncWrite + ?Sized,
    {
        let mut copy_this_round = 0usize;
        loop {
            if self.write_append_line {
                let nw = ready!(self.poll_write_line(cx, writer.as_mut()))?;
                copy_this_round += nw;

                if self.w_off >= self.r_off {
                    if self.need_flush {
                        ready!(writer.as_mut().poll_flush(cx))
                            .map_err(LimitedCopyError::WriteFailed)?;
                    }
                    return Poll::Ready(Ok(self.total));
                }
                continue;
            }

            if !self.read_done {
                if self.w_off == self.r_off {
                    // if empty, reset
                    self.w_off = 0;
                    self.r_off = 0;
                }

                // read first
                match self.poll_fill_buf(cx, reader.as_mut()) {
                    Poll::Ready(Ok(_)) => {}
                    Poll::Ready(Err(e)) => {
                        return Poll::Ready(Err(LimitedCopyError::ReadFailed(e)));
                    }
                    Poll::Pending => {
                        if self.w_off >= self.r_off {
                            // no data to write
                            if self.need_flush {
                                ready!(writer.as_mut().poll_flush(cx))
                                    .map_err(LimitedCopyError::WriteFailed)?;
                                self.need_flush = false;
                            }

                            return Poll::Pending;
                        }
                    }
                }
            }

            // If our buffer has some data, let's write it out!
            let nw = ready!(self.poll_write_buf(cx, writer.as_mut()))?;
            copy_this_round += nw;

            // yield if we have copy too much
            if copy_this_round >= self.yield_size {
                cx.waker().wake_by_ref();
                return Poll::Pending;
            }

            // If we've seen EOF and written all the data, flush out the
            // data and finish the transfer.
            if self.read_done && self.w_off == self.r_off {
                self.w_off = 0;
                if self.read_end_ok {
                    self.buf.as_mut()[0..3].copy_from_slice(b".\r\n");
                    self.line_end = 3;
                    self.r_off = 3;
                } else {
                    self.buf.as_mut()[0..5].copy_from_slice(b"\r\n.\r\n");
                    self.line_end = 5;
                    self.r_off = 5;
                }
                self.write_append_line = true;
            }
        }
    }

    pub async fn write_flush<W>(&mut self, writer: &mut W) -> Result<(), LimitedCopyError>
    where
        W: AsyncWrite + Unpin + ?Sized,
    {
        if self.write_append_line {
            if self.r_off > self.w_off {
                poll_fn(|cx| self.poll_write_line(cx, Pin::new(writer))).await?;
            }
        } else {
            poll_fn(|cx| self.poll_write_buf(cx, Pin::new(writer))).await?;
            // only write and flush the cached data
        }
        if self.need_flush {
            writer
                .flush()
                .await
                .map_err(LimitedCopyError::WriteFailed)?;
        }
        Ok(())
    }
}

pub struct TextDataEncodeTransfer<'a, R, W> {
    reader: &'a mut R,
    writer: &'a mut W,
    buf: EncodeCopyBuffer,
}

impl<'a, R, W> TextDataEncodeTransfer<'a, R, W>
where
    R: AsyncRead + Unpin,
    W: AsyncWrite + Unpin,
{
    pub fn new(reader: &'a mut R, writer: &'a mut W, config: LimitedCopyConfig) -> Self {
        TextDataEncodeTransfer {
            reader,
            writer,
            buf: EncodeCopyBuffer::new(config),
        }
    }

    #[inline]
    pub fn no_cached_data(&self) -> bool {
        self.buf.r_off == self.buf.w_off
    }

    #[inline]
    pub fn finished(&self) -> bool {
        self.buf.read_done && self.no_cached_data()
    }

    #[inline]
    pub fn copied_size(&self) -> u64 {
        self.buf.total
    }

    #[inline]
    pub fn is_active(&self) -> bool {
        self.buf.active
    }

    #[inline]
    pub fn is_idle(&self) -> bool {
        !self.buf.active
    }

    #[inline]
    pub fn reset_active(&mut self) {
        self.buf.active = false;
    }

    pub async fn write_flush(&mut self) -> Result<(), LimitedCopyError> {
        self.buf.write_flush(&mut self.writer).await
    }

    pub fn writer(self) -> &'a mut W {
        self.writer
    }
}

impl<R, W> Future for TextDataEncodeTransfer<'_, R, W>
where
    R: AsyncRead + Unpin,
    W: AsyncWrite + Unpin,
{
    type Output = Result<u64, LimitedCopyError>;

    fn poll(mut self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Self::Output> {
        let me = &mut *self;

        me.buf
            .poll_copy(cx, Pin::new(&mut *me.reader), Pin::new(&mut *me.writer))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use bytes::Bytes;
    use tokio::io::BufReader;
    use tokio_util::io::StreamReader;

    #[tokio::test]
    async fn empty() {
        let body_len: usize = 3;
        let content = b"";
        let stream = tokio_stream::iter(vec![io::Result::Ok(Bytes::from_static(content))]);
        let stream = StreamReader::new(stream);
        let mut buf_stream = BufReader::new(stream);
        let mut buf = Vec::with_capacity(64);
        let mut msg_transfer =
            TextDataEncodeTransfer::new(&mut buf_stream, &mut buf, Default::default());

        let len = (&mut msg_transfer).await.unwrap();
        assert!(msg_transfer.finished());
        assert_eq!(len, body_len as u64);
        assert_eq!(&buf, b".\r\n");
    }

    #[tokio::test]
    async fn long_with_end() {
        let body_len: usize = 23;
        let content = b"Line 1\r\n\r\n.Line 2\r\n";
        let stream = tokio_stream::iter(vec![io::Result::Ok(Bytes::from_static(content))]);
        let stream = StreamReader::new(stream);
        let mut buf_stream = BufReader::new(stream);
        let mut buf = Vec::with_capacity(64);
        let mut msg_transfer =
            TextDataEncodeTransfer::new(&mut buf_stream, &mut buf, Default::default());

        let len = (&mut msg_transfer).await.unwrap();
        assert!(msg_transfer.finished());
        assert_eq!(len, body_len as u64);
        assert_eq!(&buf, b"Line 1\r\n\r\n..Line 2\r\n.\r\n");
    }

    #[tokio::test]
    async fn long_without_end() {
        let body_len: usize = 23;
        let content = b"Line 1\r\n\r\n.Line 2";
        let stream = tokio_stream::iter(vec![io::Result::Ok(Bytes::from_static(content))]);
        let stream = StreamReader::new(stream);
        let mut buf_stream = BufReader::new(stream);
        let mut buf = Vec::with_capacity(64);
        let mut msg_transfer =
            TextDataEncodeTransfer::new(&mut buf_stream, &mut buf, Default::default());

        let len = (&mut msg_transfer).await.unwrap();
        assert!(msg_transfer.finished());
        assert_eq!(len, body_len as u64);
        assert_eq!(&buf, b"Line 1\r\n\r\n..Line 2\r\n.\r\n");
    }

    #[tokio::test]
    async fn split_with_end() {
        let body_len: usize = 23;
        let content1 = b"Line 1\r\n\r";
        let content2 = b"\n";
        let content3 = b".Line 2";
        let content4 = b"\r\n";
        let stream = tokio_stream::iter(vec![
            io::Result::Ok(Bytes::from_static(content1)),
            io::Result::Ok(Bytes::from_static(content2)),
            io::Result::Ok(Bytes::from_static(content3)),
            io::Result::Ok(Bytes::from_static(content4)),
        ]);
        let stream = StreamReader::new(stream);
        let mut buf_stream = BufReader::new(stream);
        let mut buf = Vec::with_capacity(64);
        let mut msg_transfer =
            TextDataEncodeTransfer::new(&mut buf_stream, &mut buf, Default::default());

        let len = (&mut msg_transfer).await.unwrap();
        assert!(msg_transfer.finished());
        assert_eq!(len, body_len as u64);
        assert_eq!(&buf, b"Line 1\r\n\r\n..Line 2\r\n.\r\n");
    }

    #[tokio::test]
    async fn split_without_end() {
        let body_len: usize = 23;
        let content1 = b"Line 1\r\n\r";
        let content2 = b"\n";
        let content3 = b".Line 2";
        let stream = tokio_stream::iter(vec![
            io::Result::Ok(Bytes::from_static(content1)),
            io::Result::Ok(Bytes::from_static(content2)),
            io::Result::Ok(Bytes::from_static(content3)),
        ]);
        let stream = StreamReader::new(stream);
        let mut buf_stream = BufReader::new(stream);
        let mut buf = Vec::with_capacity(64);
        let mut msg_transfer =
            TextDataEncodeTransfer::new(&mut buf_stream, &mut buf, Default::default());

        let len = (&mut msg_transfer).await.unwrap();
        assert!(msg_transfer.finished());
        assert_eq!(len, body_len as u64);
        assert_eq!(&buf, b"Line 1\r\n\r\n..Line 2\r\n.\r\n");
    }
}
