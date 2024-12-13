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
