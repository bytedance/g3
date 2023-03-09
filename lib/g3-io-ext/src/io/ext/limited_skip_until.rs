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
use std::mem;
use std::pin::Pin;
use std::task::{ready, Context, Poll};

use tokio::io::AsyncBufRead;

pub struct LimitedSkipUntil<'a, R: ?Sized> {
    reader: &'a mut R,
    delimiter: u8,
    read: usize,
    limit: usize,
}

impl<'a, R> LimitedSkipUntil<'a, R>
where
    R: AsyncBufRead + ?Sized + Unpin,
{
    pub(super) fn new(reader: &'a mut R, delimiter: u8, max_len: usize) -> Self {
        Self {
            reader,
            delimiter,
            read: 0,
            limit: max_len,
        }
    }
}

fn skip_until_internal<R: AsyncBufRead + ?Sized>(
    mut reader: Pin<&mut R>,
    cx: &mut Context<'_>,
    delimiter: u8,
    read: &mut usize,
    limit: usize,
) -> Poll<io::Result<(bool, usize)>> {
    loop {
        let (done, used) = {
            let available = ready!(reader.as_mut().poll_fill_buf(cx))?;
            if let Some(i) = memchr::memchr(delimiter, available) {
                (true, i + 1)
            } else {
                (false, available.len())
            }
        };
        reader.as_mut().consume(used);
        *read += used;
        if done {
            return if *read > limit {
                Poll::Ready(Ok((false, mem::replace(read, 0))))
            } else {
                Poll::Ready(Ok((true, mem::replace(read, 0))))
            };
        }
        if used == 0 || *read > limit {
            return Poll::Ready(Ok((false, mem::replace(read, 0))));
        }
    }
}

impl<R: AsyncBufRead + ?Sized + Unpin> Future for LimitedSkipUntil<'_, R> {
    type Output = io::Result<(bool, usize)>;

    fn poll(mut self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Self::Output> {
        let Self {
            reader,
            delimiter,
            read,
            limit,
        } = &mut *self;
        skip_until_internal(Pin::new(reader), cx, *delimiter, read, *limit)
    }
}
