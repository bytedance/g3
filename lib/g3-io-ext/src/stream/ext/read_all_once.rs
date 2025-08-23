/*
 * SPDX-License-Identifier: Apache-2.0
 * Copyright 2025 ByteDance and/or its affiliates.
 */

use std::future::Future;
use std::pin::Pin;
use std::task::{Context, Poll};

use tokio::io;
use tokio::io::{AsyncRead, ReadBuf};

pub struct ReadAllOnce<'a, R: ?Sized> {
    reader: &'a mut R,
    buf: &'a mut [u8],
}

impl<'a, R> ReadAllOnce<'a, R>
where
    R: AsyncRead + ?Sized + Unpin,
{
    pub(super) fn new(reader: &'a mut R, buf: &'a mut [u8]) -> Self {
        ReadAllOnce { reader, buf }
    }
}

fn read_all_once_internal<R: AsyncRead + ?Sized>(
    mut reader: Pin<&mut R>,
    cx: &mut Context<'_>,
    buf: &mut [u8],
) -> Poll<io::Result<usize>> {
    let mut buf = ReadBuf::new(buf);
    let mut quit_on_pending = false;
    loop {
        if buf.remaining() == 0 {
            return Poll::Ready(Ok(buf.filled().len()));
        }
        let old_filled_len = buf.filled().len();
        match reader.as_mut().poll_read(cx, &mut buf) {
            Poll::Ready(Ok(_)) => {
                quit_on_pending = true;
                let filled_len = buf.filled().len();
                if filled_len == 0 {
                    return Poll::Ready(Ok(0));
                }
                if filled_len == old_filled_len {
                    return Poll::Ready(Ok(filled_len));
                }
            }
            Poll::Ready(Err(e)) => return Poll::Ready(Err(e)),
            Poll::Pending => {
                return if quit_on_pending {
                    Poll::Ready(Ok(buf.filled().len()))
                } else {
                    Poll::Pending
                };
            }
        }
    }
}

impl<R> Future for ReadAllOnce<'_, R>
where
    R: AsyncRead + ?Sized + Unpin,
{
    type Output = io::Result<usize>;

    fn poll(mut self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Self::Output> {
        let ReadAllOnce { reader, buf } = &mut *self;
        read_all_once_internal(Pin::new(reader), cx, buf)
    }
}
