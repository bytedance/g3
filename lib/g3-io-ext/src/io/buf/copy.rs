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

use std::future::poll_fn;
use std::io;
use std::pin::Pin;
use std::task::{ready, Context, Poll};

use tokio::io::{AsyncBufRead, AsyncWrite, AsyncWriteExt};

use crate::{LimitedCopyConfig, LimitedCopyError};

pub struct LimitedBufCopy<'a, R: ?Sized, W: ?Sized> {
    reader: &'a mut R,
    writer: &'a mut W,
    yield_size: usize,
    total_write: u64,
    buf_size: usize,
    read_done: bool,
    need_flush: bool,
    active: bool,
}

impl<'a, R, W> LimitedBufCopy<'a, R, W>
where
    R: AsyncBufRead + Unpin + ?Sized,
    W: AsyncWrite + Unpin + ?Sized,
{
    pub fn new(reader: &'a mut R, writer: &'a mut W, config: &LimitedCopyConfig) -> Self {
        LimitedBufCopy {
            reader,
            writer,
            yield_size: config.yield_size(),
            total_write: 0,
            buf_size: 0,
            read_done: false,
            need_flush: false,
            active: false,
        }
    }

    #[inline]
    pub fn no_cached_data(&self) -> bool {
        self.buf_size == 0
    }

    #[inline]
    pub fn finished(&self) -> bool {
        self.read_done
    }

    #[inline]
    pub fn copied_size(&self) -> u64 {
        self.total_write
    }

    #[inline]
    pub fn is_active(&self) -> bool {
        self.active
    }

    #[inline]
    pub fn is_idle(&self) -> bool {
        !self.active
    }

    #[inline]
    pub fn reset_active(&mut self) {
        self.active = false;
    }

    fn poll_write_cache(&mut self, cx: &mut Context<'_>) -> Poll<Result<(), LimitedCopyError>> {
        loop {
            match Pin::new(&mut self.reader).poll_fill_buf(cx) {
                Poll::Ready(Ok(buf)) => {
                    self.buf_size = buf.len();
                    if buf.is_empty() {
                        self.read_done = true;
                        return Poll::Ready(Ok(()));
                    }
                    let i = ready!(Pin::new(&mut self.writer).poll_write(cx, buf))
                        .map_err(LimitedCopyError::WriteFailed)?;
                    self.need_flush = true;
                    self.active = true;
                    self.buf_size -= i;
                    self.total_write += i as u64;
                    Pin::new(&mut *self.reader).consume(i);
                }
                Poll::Ready(Err(e)) => return Poll::Ready(Err(LimitedCopyError::ReadFailed(e))),
                Poll::Pending => return Poll::Ready(Ok(())),
            }
        }
    }

    pub async fn write_flush(&mut self) -> Result<(), LimitedCopyError> {
        if self.read_done {
            return Ok(());
        }

        if self.buf_size > 0 {
            poll_fn(|cx| self.poll_write_cache(cx)).await?;
        }

        if self.need_flush {
            self.writer
                .flush()
                .await
                .map_err(LimitedCopyError::WriteFailed)?;
        }

        Ok(())
    }
}

impl<R, W> Future for LimitedBufCopy<'_, R, W>
where
    R: AsyncBufRead + Unpin + ?Sized,
    W: AsyncWrite + Unpin + ?Sized,
{
    type Output = Result<u64, LimitedCopyError>;

    fn poll(mut self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Self::Output> {
        let mut copy_this_round = 0;
        loop {
            let me = &mut *self;
            let buffer = match Pin::new(&mut *me.reader).poll_fill_buf(cx) {
                Poll::Ready(Ok(buffer)) => {
                    me.buf_size = buffer.len();
                    if buffer.is_empty() {
                        if self.need_flush {
                            ready!(Pin::new(&mut self.writer).poll_flush(cx))
                                .map_err(LimitedCopyError::WriteFailed)?;
                        }
                        self.read_done = true;
                        return Poll::Ready(Ok(self.total_write));
                    }
                    buffer
                }
                Poll::Ready(Err(e)) => return Poll::Ready(Err(LimitedCopyError::ReadFailed(e))),
                Poll::Pending => {
                    if self.need_flush {
                        ready!(Pin::new(&mut self.writer).poll_flush(cx))
                            .map_err(LimitedCopyError::WriteFailed)?;
                    }
                    return Poll::Pending;
                }
            };

            let i = ready!(Pin::new(&mut *me.writer).poll_write(cx, buffer))
                .map_err(LimitedCopyError::WriteFailed)?;
            if i == 0 {
                return Poll::Ready(Err(LimitedCopyError::WriteFailed(
                    io::ErrorKind::WriteZero.into(),
                )));
            }
            self.need_flush = true;
            self.active = true;
            self.buf_size -= i;
            self.total_write += i as u64;
            Pin::new(&mut *self.reader).consume(i);

            copy_this_round += i;
            if copy_this_round >= self.yield_size {
                cx.waker().wake_by_ref();
                return Poll::Pending;
            }
        }
    }
}
