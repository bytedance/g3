/*
 * SPDX-License-Identifier: Apache-2.0
 * Copyright 2024-2025 ByteDance and/or its affiliates.
 */

use std::io;
use std::mem;
use std::pin::Pin;
use std::task::{Context, Poll, ready};

use pin_project_lite::pin_project;
use tokio::io::AsyncWrite;

pin_project! {
    #[derive(Debug)]
    #[must_use = "futures do nothing unless you `.await` or poll them"]
    pub struct WriteAllFlush<'a, W: ?Sized> {
        writer: &'a mut W,
        buf: &'a [u8],
        flush_done: bool,
    }
}

impl<'a, W> WriteAllFlush<'a, W>
where
    W: AsyncWrite + Unpin + ?Sized,
{
    pub(crate) fn new(writer: &'a mut W, buf: &'a [u8]) -> Self {
        WriteAllFlush {
            writer,
            buf,
            flush_done: false,
        }
    }
}

impl<W> Future for WriteAllFlush<'_, W>
where
    W: AsyncWrite + Unpin + ?Sized,
{
    type Output = io::Result<()>;

    fn poll(self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<io::Result<()>> {
        let me = self.project();
        while !me.buf.is_empty() {
            let n = ready!(Pin::new(&mut *me.writer).poll_write(cx, me.buf))?;
            {
                let (_, rest) = mem::take(&mut *me.buf).split_at(n);
                *me.buf = rest;
            }
            if n == 0 {
                return Poll::Ready(Err(io::ErrorKind::WriteZero.into()));
            }
        }

        if !*me.flush_done {
            ready!(Pin::new(&mut *me.writer).poll_flush(cx))?;
            *me.flush_done = true;
        }

        Poll::Ready(Ok(()))
    }
}
