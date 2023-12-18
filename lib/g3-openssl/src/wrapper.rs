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
use std::task::{Context, Poll};

use tokio::io::{AsyncRead, AsyncWrite, ReadBuf};

pub(crate) struct SslIoWrapper<S> {
    io: S,
    cx: usize,
}

impl<S> SslIoWrapper<S> {
    pub(crate) fn new(io: S) -> Self {
        SslIoWrapper { io, cx: 0 }
    }

    #[inline]
    pub(crate) fn set_cx(&mut self, cx: &mut Context<'_>) {
        self.cx = cx as *mut _ as usize;
    }

    #[inline]
    pub(crate) fn get_mut(&mut self) -> &mut S {
        &mut self.io
    }

    #[inline]
    pub(crate) fn get_pin_mut(&mut self) -> Pin<&mut S> {
        unsafe { Pin::new_unchecked(&mut self.io) }
    }

    unsafe fn parts(&mut self) -> (Pin<&mut S>, &mut Context<'_>) {
        debug_assert_ne!(self.cx, 0);
        let stream = Pin::new_unchecked(&mut self.io);
        let context = &mut *(self.cx as *mut _);
        (stream, context)
    }
}

impl<S: AsyncRead> Read for SslIoWrapper<S> {
    fn read(&mut self, buf: &mut [u8]) -> io::Result<usize> {
        let (stream, cx) = unsafe { self.parts() };
        let mut buf = ReadBuf::new(buf);
        match stream.poll_read(cx, &mut buf) {
            Poll::Ready(Ok(_)) => Ok(buf.filled().len()),
            Poll::Ready(Err(e)) => Err(e),
            Poll::Pending => Err(io::Error::from(io::ErrorKind::WouldBlock)),
        }
    }
}

impl<S: AsyncWrite> Write for SslIoWrapper<S> {
    fn write(&mut self, buf: &[u8]) -> io::Result<usize> {
        let (stream, cx) = unsafe { self.parts() };
        match stream.poll_write(cx, buf) {
            Poll::Ready(r) => r,
            Poll::Pending => Err(io::Error::from(io::ErrorKind::WouldBlock)),
        }
    }

    fn flush(&mut self) -> io::Result<()> {
        let (stream, cx) = unsafe { self.parts() };
        match stream.poll_flush(cx) {
            Poll::Ready(r) => r,
            Poll::Pending => Err(io::Error::from(io::ErrorKind::WouldBlock)),
        }
    }
}
