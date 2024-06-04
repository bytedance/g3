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

use std::future::Future;
use std::io;
use std::mem;
use std::pin::Pin;
use std::task::{ready, Context, Poll};

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
