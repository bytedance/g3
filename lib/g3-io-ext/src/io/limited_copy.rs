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
use std::io;
use std::pin::Pin;
use std::task::{ready, Context, Poll};

use thiserror::Error;
use tokio::io::{AsyncRead, AsyncWrite, AsyncWriteExt, ReadBuf};

const DEFAULT_COPY_BUFFER_SIZE: usize = 16 * 1024; // 16KB
const MINIMAL_COPY_BUFFER_SIZE: usize = 4 * 1024; // 4KB
const DEFAULT_COPY_YIELD_SIZE: usize = 1024 * 1024; // 1MB
const MINIMAL_COPY_YIELD_SIZE: usize = 256 * 1024; // 256KB

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct LimitedCopyConfig {
    buffer_size: usize,
    yield_size: usize,
}

impl Default for LimitedCopyConfig {
    fn default() -> Self {
        LimitedCopyConfig {
            buffer_size: DEFAULT_COPY_BUFFER_SIZE,
            yield_size: DEFAULT_COPY_YIELD_SIZE,
        }
    }
}

impl LimitedCopyConfig {
    pub fn set_buffer_size(&mut self, buffer_size: usize) {
        self.buffer_size = buffer_size.max(MINIMAL_COPY_BUFFER_SIZE);
    }

    #[inline]
    pub fn buffer_size(&self) -> usize {
        self.buffer_size
    }

    pub fn set_yield_size(&mut self, yield_size: usize) {
        self.yield_size = yield_size.max(MINIMAL_COPY_YIELD_SIZE);
    }

    #[inline]
    pub fn yield_size(&self) -> usize {
        self.yield_size
    }
}

#[derive(Error, Debug)]
pub enum LimitedCopyError {
    #[error("read failed: {0:?}")]
    ReadFailed(io::Error),
    #[error("write failed: {0:?}")]
    WriteFailed(io::Error),
}

#[derive(Debug)]
struct LimitedCopyBuffer {
    read_done: bool,
    buf: Box<[u8]>,
    yield_size: usize,
    r_off: usize,
    w_off: usize,
    total: u64,
    need_flush: bool,
    active: bool,
}

impl LimitedCopyBuffer {
    fn new(config: &LimitedCopyConfig) -> Self {
        LimitedCopyBuffer {
            read_done: false,
            buf: vec![0; config.buffer_size].into_boxed_slice(),
            yield_size: config.yield_size,
            r_off: 0,
            w_off: 0,
            total: 0,
            need_flush: false,
            active: false,
        }
    }

    fn with_data(config: &LimitedCopyConfig, mut buf: Vec<u8>) -> Self {
        let r_off = buf.len();
        if buf.capacity() < config.buffer_size {
            buf.resize(config.buffer_size, 0);
        } else {
            buf.resize(buf.capacity(), 0);
        }
        LimitedCopyBuffer {
            read_done: false,
            buf: buf.into_boxed_slice(),
            yield_size: config.yield_size,
            r_off,
            w_off: 0,
            total: 0,
            need_flush: false,
            active: true, // as we have data
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
        let mut buf = ReadBuf::new(&mut self.buf);
        buf.set_filled(self.r_off);

        let res = reader.poll_read(cx, &mut buf);
        if let Poll::Ready(Ok(_)) = res {
            let filled_len = buf.filled().len();
            if self.r_off == filled_len {
                self.read_done = true;
            } else {
                self.r_off = filled_len;
                self.active = true;
            }
        }
        res
    }

    fn poll_write_buf<R, W>(
        &mut self,
        cx: &mut Context<'_>,
        reader: Pin<&mut R>,
        writer: Pin<&mut W>,
    ) -> Poll<Result<usize, LimitedCopyError>>
    where
        R: AsyncRead + ?Sized,
        W: AsyncWrite + ?Sized,
    {
        match writer.poll_write(cx, &self.buf[self.w_off..self.r_off]) {
            Poll::Pending => {
                // Top up the buffer towards full if we can read a bit more
                // data - this should improve the chances of a large write
                if !self.read_done && self.r_off < self.buf.len() {
                    let left = self.r_off - self.w_off;
                    if left < self.w_off {
                        // copy small data to the begin of the buffer, so we can read more data
                        unsafe {
                            let ptr = self.buf.as_mut_ptr();
                            let src_ptr = ptr.add(self.w_off);
                            std::ptr::copy_nonoverlapping(src_ptr, ptr, left);
                        }
                        self.w_off = 0;
                        self.r_off = left;
                    }
                    ready!(self.poll_fill_buf(cx, reader)).map_err(LimitedCopyError::ReadFailed)?;
                }
                Poll::Pending
            }
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
            if !self.read_done {
                if self.w_off == self.r_off {
                    // if empty, reset
                    self.w_off = 0;
                    self.r_off = 0;
                }

                if self.r_off < self.buf.len() {
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
            }

            // If our buffer has some data, let's write it out!
            while self.w_off < self.r_off {
                // return if write blocked. no need to try flush
                let i = ready!(self.poll_write_buf(cx, reader.as_mut(), writer.as_mut()))?;
                copy_this_round += i;
            }

            // yield if we have copy too much
            if copy_this_round >= self.yield_size {
                cx.waker().wake_by_ref();
                return Poll::Pending;
            }

            // If we've seen EOF and written all the data, flush out the
            // data and finish the transfer.
            if self.read_done && self.w_off == self.r_off {
                if self.need_flush {
                    ready!(writer.as_mut().poll_flush(cx))
                        .map_err(LimitedCopyError::WriteFailed)?;
                }
                return Poll::Ready(Ok(self.total));
            }
        }
    }

    pub async fn write_flush<W>(&mut self, writer: &mut W) -> Result<(), LimitedCopyError>
    where
        W: AsyncWrite + Unpin + ?Sized,
    {
        if self.w_off < self.r_off {
            writer
                .write_all(&self.buf[self.w_off..self.r_off])
                .await
                .map_err(LimitedCopyError::WriteFailed)?;
            self.total += (self.r_off - self.w_off) as u64;
            self.w_off = self.r_off;
            writer
                .flush()
                .await
                .map_err(LimitedCopyError::WriteFailed)?;
        }
        Ok(())
    }
}

#[derive(Debug)]
pub struct LimitedCopy<'a, R: ?Sized, W: ?Sized> {
    reader: &'a mut R,
    writer: &'a mut W,
    buf: LimitedCopyBuffer,
}

impl<'a, R, W> LimitedCopy<'a, R, W>
where
    R: AsyncRead + Unpin + ?Sized,
    W: AsyncWrite + Unpin + ?Sized,
{
    pub fn new(reader: &'a mut R, writer: &'a mut W, config: &LimitedCopyConfig) -> Self {
        LimitedCopy {
            reader,
            writer,
            buf: LimitedCopyBuffer::new(config),
        }
    }

    pub fn with_data(
        reader: &'a mut R,
        writer: &'a mut W,
        config: &LimitedCopyConfig,
        data: Vec<u8>,
    ) -> Self {
        LimitedCopy {
            reader,
            writer,
            buf: LimitedCopyBuffer::with_data(config, data),
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
}

impl<R, W> Future for LimitedCopy<'_, R, W>
where
    R: AsyncRead + Unpin + ?Sized,
    W: AsyncWrite + Unpin + ?Sized,
{
    type Output = Result<u64, LimitedCopyError>;

    fn poll(mut self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Result<u64, LimitedCopyError>> {
        let me = &mut *self;

        me.buf
            .poll_copy(cx, Pin::new(&mut *me.reader), Pin::new(&mut *me.writer))
    }
}

#[derive(Debug)]
pub struct ROwnedLimitedCopy<'a, R, W: ?Sized> {
    reader: R,
    writer: &'a mut W,
    buf: LimitedCopyBuffer,
}

impl<'a, R, W> ROwnedLimitedCopy<'a, R, W>
where
    R: AsyncRead + Unpin,
    W: AsyncWrite + Unpin + ?Sized,
{
    pub fn new(reader: R, writer: &'a mut W, config: LimitedCopyConfig) -> Self {
        ROwnedLimitedCopy {
            reader,
            writer,
            buf: LimitedCopyBuffer::new(&config),
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

impl<R, W> Future for ROwnedLimitedCopy<'_, R, W>
where
    R: AsyncRead + Unpin,
    W: AsyncWrite + Unpin + ?Sized,
{
    type Output = Result<u64, LimitedCopyError>;

    fn poll(mut self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Result<u64, LimitedCopyError>> {
        let me = &mut *self;

        me.buf
            .poll_copy(cx, Pin::new(&mut me.reader), Pin::new(&mut *me.writer))
    }
}
