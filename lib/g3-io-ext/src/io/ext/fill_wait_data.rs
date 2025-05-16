/*
 * SPDX-License-Identifier: Apache-2.0
 * Copyright 2023-2025 ByteDance and/or its affiliates.
 */

use std::io;
use std::pin::Pin;
use std::task::{Context, Poll, ready};

use tokio::io::AsyncBufRead;

pub struct FillWaitData<'a, R: ?Sized> {
    reader: &'a mut R,
}

impl<'a, R> FillWaitData<'a, R>
where
    R: AsyncBufRead + ?Sized + Unpin,
{
    pub(super) fn new(reader: &'a mut R) -> Self {
        Self { reader }
    }
}

fn fill_wait_data<R: AsyncBufRead + ?Sized>(
    reader: Pin<&mut R>,
    cx: &mut Context<'_>,
) -> Poll<io::Result<bool>> {
    let buf = ready!(reader.poll_fill_buf(cx))?;
    if buf.is_empty() {
        Poll::Ready(Ok(false))
    } else {
        Poll::Ready(Ok(true))
    }
}

impl<R: AsyncBufRead + ?Sized + Unpin> Future for FillWaitData<'_, R> {
    type Output = io::Result<bool>;

    fn poll(mut self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Self::Output> {
        let Self { reader } = &mut *self;
        fill_wait_data(Pin::new(reader), cx)
    }
}
