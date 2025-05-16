/*
 * SPDX-License-Identifier: Apache-2.0
 * Copyright 2023-2025 ByteDance and/or its affiliates.
 */

use std::io::{self, Read, Write};
use std::pin::Pin;
use std::task::{Context, Poll, Waker};

use tokio::io::{AsyncRead, AsyncWrite, ReadBuf};

pub(crate) struct SslIoWrapper<S> {
    io: S,
    waker: Option<Waker>,
}

impl<S> SslIoWrapper<S> {
    pub(crate) fn new(io: S) -> Self {
        SslIoWrapper { io, waker: None }
    }

    #[inline]
    pub(crate) fn set_cx(&mut self, cx: &mut Context<'_>) {
        self.waker = Some(cx.waker().clone());
    }

    #[inline]
    pub(crate) fn get_mut(&mut self) -> &mut S {
        &mut self.io
    }

    #[inline]
    pub(crate) fn get_pin_mut(&mut self) -> Pin<&mut S>
    where
        S: Unpin,
    {
        Pin::new(&mut self.io)
    }

    fn with_context<F, R>(&mut self, mut f: F) -> R
    where
        F: FnMut(Pin<&mut S>, &mut Context<'_>) -> R,
        S: Unpin,
    {
        let stream = Pin::new(&mut self.io);
        let mut context =
            Context::from_waker(self.waker.as_ref().expect("async context waker is not set"));
        f(stream, &mut context)
    }
}

impl<S: AsyncRead + Unpin> Read for SslIoWrapper<S> {
    fn read(&mut self, buf: &mut [u8]) -> io::Result<usize> {
        self.with_context(|stream, cx| {
            let mut buf = ReadBuf::new(buf);
            match stream.poll_read(cx, &mut buf) {
                Poll::Ready(Ok(_)) => Ok(buf.filled().len()),
                Poll::Ready(Err(e)) => Err(e),
                Poll::Pending => Err(io::Error::from(io::ErrorKind::WouldBlock)),
            }
        })
    }
}

impl<S: AsyncWrite + Unpin> Write for SslIoWrapper<S> {
    fn write(&mut self, buf: &[u8]) -> io::Result<usize> {
        self.with_context(|stream, cx| match stream.poll_write(cx, buf) {
            Poll::Ready(r) => r,
            Poll::Pending => Err(io::Error::from(io::ErrorKind::WouldBlock)),
        })
    }

    fn flush(&mut self) -> io::Result<()> {
        self.with_context(|stream, cx| match stream.poll_flush(cx) {
            Poll::Ready(r) => r,
            Poll::Pending => Err(io::Error::from(io::ErrorKind::WouldBlock)),
        })
    }
}
