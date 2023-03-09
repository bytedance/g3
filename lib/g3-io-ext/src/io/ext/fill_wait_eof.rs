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

use tokio::io::AsyncBufRead;

pub struct FillWaitEof<'a, R: ?Sized> {
    reader: &'a mut R,
}

impl<'a, R> FillWaitEof<'a, R>
where
    R: AsyncBufRead + ?Sized + Unpin,
{
    pub(super) fn new(reader: &'a mut R) -> Self {
        Self { reader }
    }
}

fn fill_wait_eof<R: AsyncBufRead + ?Sized>(
    reader: Pin<&mut R>,
    cx: &mut Context<'_>,
) -> Poll<io::Result<()>> {
    let buf = ready!(reader.poll_fill_buf(cx))?;
    if buf.is_empty() {
        Poll::Ready(Ok(()))
    } else {
        // there are data in buf, will never ready
        Poll::Pending
    }
}

impl<R: AsyncBufRead + ?Sized + Unpin> Future for FillWaitEof<'_, R> {
    type Output = io::Result<()>;

    fn poll(mut self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Self::Output> {
        let Self { reader } = &mut *self;
        fill_wait_eof(Pin::new(reader), cx)
    }
}
