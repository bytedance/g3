/*
 * SPDX-License-Identifier: Apache-2.0
 * Copyright 2023-2025 ByteDance and/or its affiliates.
 */

use std::io;
use std::pin::Pin;
use std::task::{Context, Poll, ready};

use thiserror::Error;
use tokio::io::{AsyncRead, AsyncWrite, AsyncWriteExt, ReadBuf};

const DEFAULT_COPY_BUFFER_SIZE: usize = 16 * 1024; // 16KB
const MINIMAL_COPY_BUFFER_SIZE: usize = 4 * 1024; // 4KB
const MINIMAL_READ_BUFFER_SIZE: usize = 256; // 256B
const DEFAULT_COPY_YIELD_SIZE: usize = 1024 * 1024; // 1MB
const MINIMAL_COPY_YIELD_SIZE: usize = 256 * 1024; // 256KB

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct StreamCopyConfig {
    buffer_size: usize,
    yield_size: usize,
}

impl Default for StreamCopyConfig {
    fn default() -> Self {
        StreamCopyConfig {
            buffer_size: DEFAULT_COPY_BUFFER_SIZE,
            yield_size: DEFAULT_COPY_YIELD_SIZE,
        }
    }
}

impl StreamCopyConfig {
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
pub enum StreamCopyError {
    #[error("read failed: {0:?}")]
    ReadFailed(io::Error),
    #[error("write failed: {0:?}")]
    WriteFailed(io::Error),
}

#[derive(Debug)]
struct StreamCopyBuffer {
    read_done: bool,
    buf: Box<[u8]>,
    yield_size: usize,
    r_off: usize,
    w_off: usize,
    total_read: u64,
    total_write: u64,
    need_flush: bool,
    active: bool,
}

impl StreamCopyBuffer {
    fn new(config: &StreamCopyConfig) -> Self {
        StreamCopyBuffer {
            read_done: false,
            buf: vec![0; config.buffer_size].into_boxed_slice(),
            yield_size: config.yield_size,
            r_off: 0,
            w_off: 0,
            total_read: 0,
            total_write: 0,
            need_flush: false,
            active: false,
        }
    }

    fn with_data(config: &StreamCopyConfig, mut buf: Vec<u8>) -> Self {
        let r_off = buf.len();
        if buf.capacity() < config.buffer_size {
            buf.resize(config.buffer_size, 0);
        } else {
            buf.resize(buf.capacity(), 0);
        }
        StreamCopyBuffer {
            read_done: false,
            buf: buf.into_boxed_slice(),
            yield_size: config.yield_size,
            r_off,
            w_off: 0,
            total_read: 0,
            total_write: 0,
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
        let mut read_buf = ReadBuf::new(&mut self.buf[self.r_off..]);
        let res = reader.poll_read(cx, &mut read_buf);
        if let Poll::Ready(Ok(_)) = res {
            let nr = read_buf.filled().len();
            if nr == 0 {
                self.read_done = true;
            } else {
                self.r_off += nr;
                self.total_read += nr as u64;
                self.active = true;
            }
        }
        res
    }

    fn check_move_cache(&mut self) {
        let left = self.r_off - self.w_off;
        if left <= self.w_off {
            // copy small data to the start of the buffer, so we can read more data
            unsafe {
                let ptr = self.buf.as_mut_ptr();
                let src_ptr = ptr.add(self.w_off);
                std::ptr::copy_nonoverlapping(src_ptr, ptr, left);
            }
            self.w_off = 0;
            self.r_off = left;
        }
    }

    fn poll_write_buf<R, W>(
        &mut self,
        cx: &mut Context<'_>,
        reader: Pin<&mut R>,
        writer: Pin<&mut W>,
    ) -> Poll<Result<usize, StreamCopyError>>
    where
        R: AsyncRead + ?Sized,
        W: AsyncWrite + ?Sized,
    {
        match writer.poll_write(cx, &self.buf[self.w_off..self.r_off]) {
            Poll::Pending => {
                // Top up the buffer towards full if we can read a bit more
                // data - this should improve the chances of a large write
                if !self.read_done {
                    self.check_move_cache();
                    if self.r_off + MINIMAL_READ_BUFFER_SIZE <= self.buf.len() {
                        // avoid too small read
                        ready!(self.poll_fill_buf(cx, reader))
                            .map_err(StreamCopyError::ReadFailed)?;
                    }
                }
                Poll::Pending
            }
            Poll::Ready(Err(e)) => Poll::Ready(Err(StreamCopyError::WriteFailed(e))),
            Poll::Ready(Ok(0)) => Poll::Ready(Err(StreamCopyError::WriteFailed(io::Error::new(
                io::ErrorKind::WriteZero,
                "write zero byte into writer",
            )))),
            Poll::Ready(Ok(n)) => {
                self.w_off += n;
                self.total_write += n as u64;
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
    ) -> Poll<Result<u64, StreamCopyError>>
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

                if self.w_off != 0 {
                    self.check_move_cache();
                }
                if self.r_off < self.buf.len() {
                    // read first
                    match self.poll_fill_buf(cx, reader.as_mut()) {
                        Poll::Ready(Ok(_)) => {}
                        Poll::Ready(Err(e)) => {
                            return Poll::Ready(Err(StreamCopyError::ReadFailed(e)));
                        }
                        Poll::Pending => {
                            if self.w_off >= self.r_off {
                                // no data to write
                                if self.need_flush {
                                    // trigger flush, no need to flush again on pending
                                    self.need_flush = false;
                                    ready!(writer.as_mut().poll_flush(cx))
                                        .map_err(StreamCopyError::WriteFailed)?;
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

            // If we've seen EOF and written all the data, flush out the
            // data and finish the transfer.
            if self.read_done {
                if self.need_flush {
                    ready!(writer.as_mut().poll_flush(cx)).map_err(StreamCopyError::WriteFailed)?;
                }
                return Poll::Ready(Ok(self.total_write));
            }

            // yield if we have copy too much
            if copy_this_round >= self.yield_size {
                cx.waker().wake_by_ref();
                return Poll::Pending;
            }
        }
    }

    pub async fn write_flush<W>(&mut self, writer: &mut W) -> Result<(), StreamCopyError>
    where
        W: AsyncWrite + Unpin + ?Sized,
    {
        if self.w_off < self.r_off {
            writer
                .write_all(&self.buf[self.w_off..self.r_off])
                .await
                .map_err(StreamCopyError::WriteFailed)?;
            self.total_write += (self.r_off - self.w_off) as u64;
            self.w_off = self.r_off;
            writer.flush().await.map_err(StreamCopyError::WriteFailed)?;
        }
        Ok(())
    }
}

#[derive(Debug)]
pub struct StreamCopy<'a, R: ?Sized, W: ?Sized> {
    reader: &'a mut R,
    writer: &'a mut W,
    buf: StreamCopyBuffer,
}

impl<'a, R, W> StreamCopy<'a, R, W>
where
    R: AsyncRead + Unpin + ?Sized,
    W: AsyncWrite + Unpin + ?Sized,
{
    pub fn new(reader: &'a mut R, writer: &'a mut W, config: &StreamCopyConfig) -> Self {
        StreamCopy {
            reader,
            writer,
            buf: StreamCopyBuffer::new(config),
        }
    }

    pub fn with_data(
        reader: &'a mut R,
        writer: &'a mut W,
        config: &StreamCopyConfig,
        data: Vec<u8>,
    ) -> Self {
        StreamCopy {
            reader,
            writer,
            buf: StreamCopyBuffer::with_data(config, data),
        }
    }

    pub fn writer(&mut self) -> &mut W {
        self.writer
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
    pub fn read_size(&self) -> u64 {
        self.buf.total_read
    }

    #[inline]
    pub fn copied_size(&self) -> u64 {
        self.buf.total_write
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

    pub async fn write_flush(&mut self) -> Result<(), StreamCopyError> {
        self.buf.write_flush(&mut self.writer).await
    }
}

impl<R, W> Future for StreamCopy<'_, R, W>
where
    R: AsyncRead + Unpin + ?Sized,
    W: AsyncWrite + Unpin + ?Sized,
{
    type Output = Result<u64, StreamCopyError>;

    fn poll(mut self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Result<u64, StreamCopyError>> {
        let me = &mut *self;

        me.buf
            .poll_copy(cx, Pin::new(&mut *me.reader), Pin::new(&mut *me.writer))
    }
}

#[derive(Debug)]
pub struct ROwnedStreamCopy<'a, R, W: ?Sized> {
    reader: R,
    writer: &'a mut W,
    buf: StreamCopyBuffer,
}

impl<'a, R, W> ROwnedStreamCopy<'a, R, W>
where
    R: AsyncRead + Unpin,
    W: AsyncWrite + Unpin + ?Sized,
{
    pub fn new(reader: R, writer: &'a mut W, config: StreamCopyConfig) -> Self {
        ROwnedStreamCopy {
            reader,
            writer,
            buf: StreamCopyBuffer::new(&config),
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
        self.buf.total_write
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

    pub async fn write_flush(&mut self) -> Result<(), StreamCopyError> {
        self.buf.write_flush(&mut self.writer).await
    }

    pub fn writer(self) -> &'a mut W {
        self.writer
    }
}

impl<R, W> Future for ROwnedStreamCopy<'_, R, W>
where
    R: AsyncRead + Unpin,
    W: AsyncWrite + Unpin + ?Sized,
{
    type Output = Result<u64, StreamCopyError>;

    fn poll(mut self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Result<u64, StreamCopyError>> {
        let me = &mut *self;

        me.buf
            .poll_copy(cx, Pin::new(&mut me.reader), Pin::new(&mut *me.writer))
    }
}
