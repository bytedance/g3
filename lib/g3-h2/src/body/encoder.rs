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

use bytes::{Buf, Bytes, BytesMut};
use h2::SendStream;
use thiserror::Error;
use tokio::io::{AsyncRead, ReadBuf};

use g3_io_ext::LimitedCopyConfig;

#[derive(Debug, Error)]
pub enum H2StreamBodyEncodeTransferError {
    #[error("read error: {0:?}")]
    ReadError(io::Error),
    #[error("send data failed: {0}")]
    SendDataFailed(h2::Error),
}

struct H2BodyEncodeTransferInternal {
    buffer_size: usize,
    yield_size: usize,
    chunk: Option<Bytes>,
    active: bool,
}

impl H2BodyEncodeTransferInternal {
    fn new(copy_config: &LimitedCopyConfig) -> Self {
        H2BodyEncodeTransferInternal {
            buffer_size: copy_config.buffer_size(),
            yield_size: copy_config.yield_size(),
            chunk: None,
            active: false,
        }
    }

    #[inline]
    fn is_idle(&self) -> bool {
        !self.active
    }

    #[inline]
    fn is_active(&self) -> bool {
        self.active
    }

    fn reset_active(&mut self) {
        self.active = false;
    }

    fn no_cached_data(&self) -> bool {
        self.chunk.is_none()
    }

    fn poll_encode<R>(
        &mut self,
        cx: &mut Context<'_>,
        mut reader: Pin<&mut R>,
        send_stream: &mut SendStream<Bytes>,
    ) -> Poll<Result<(), H2StreamBodyEncodeTransferError>>
    where
        R: AsyncRead + Unpin,
    {
        let mut copy_this_round = 0usize;

        loop {
            if let Some(mut chunk) = self.chunk.take() {
                match send_stream.poll_capacity(cx) {
                    Poll::Ready(Some(Ok(n))) => {
                        self.active = true;
                        let to_send = chunk.split_to(n);
                        send_stream
                            .send_data(to_send, false)
                            .map_err(H2StreamBodyEncodeTransferError::SendDataFailed)?;
                        if chunk.has_remaining() {
                            self.chunk = Some(chunk);
                        }

                        copy_this_round += n;
                        if copy_this_round >= self.yield_size {
                            cx.waker().wake_by_ref();
                            return Poll::Pending;
                        }
                    }
                    Poll::Ready(Some(Err(e))) => {
                        self.chunk = Some(chunk);
                        return Poll::Ready(Err(H2StreamBodyEncodeTransferError::SendDataFailed(
                            e,
                        )));
                    }
                    Poll::Ready(None) => {
                        // only possible if the reserve capacity is 0 or not set
                        unreachable!()
                    }
                    Poll::Pending => {
                        self.chunk = Some(chunk);
                        return Poll::Pending;
                    }
                }
            } else {
                let mut data = BytesMut::zeroed(self.buffer_size);
                let mut buf = ReadBuf::new(&mut data);
                ready!(reader.as_mut().poll_read(cx, &mut buf))
                    .map_err(H2StreamBodyEncodeTransferError::ReadError)?;
                let nr = buf.filled().len();
                if nr == 0 {
                    return Poll::Ready(Ok(()));
                }
                let chunk = data.split_to(nr).freeze();
                self.chunk = Some(chunk);
                self.active = true;
                send_stream.reserve_capacity(nr);
            }
        }
    }
}

pub struct H2BodyEncodeTransfer<'a, R> {
    reader: &'a mut R,
    send_stream: &'a mut SendStream<Bytes>,
    internal: H2BodyEncodeTransferInternal,
}

impl<'a, R> H2BodyEncodeTransfer<'a, R> {
    pub fn new(
        reader: &'a mut R,
        send_stream: &'a mut SendStream<Bytes>,
        copy_config: &LimitedCopyConfig,
    ) -> Self {
        H2BodyEncodeTransfer {
            reader,
            send_stream,
            internal: H2BodyEncodeTransferInternal::new(copy_config),
        }
    }

    #[inline]
    pub fn is_idle(&self) -> bool {
        self.internal.is_idle()
    }

    #[inline]
    pub fn is_active(&self) -> bool {
        self.internal.is_active()
    }

    pub fn reset_active(&mut self) {
        self.internal.reset_active()
    }

    pub fn no_cached_data(&self) -> bool {
        self.internal.no_cached_data()
    }

    pub fn into_io(self) -> (&'a mut R, &'a mut SendStream<Bytes>) {
        (self.reader, self.send_stream)
    }
}

impl<'a, R> Future for H2BodyEncodeTransfer<'a, R>
where
    R: AsyncRead + Unpin,
{
    type Output = Result<(), H2StreamBodyEncodeTransferError>;

    fn poll(mut self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Self::Output> {
        let me = &mut *self;

        me.internal
            .poll_encode(cx, Pin::new(&mut me.reader), me.send_stream)
    }
}

pub struct ROwnedH2BodyEncodeTransfer<'a, R> {
    reader: R,
    send_stream: &'a mut SendStream<Bytes>,
    internal: H2BodyEncodeTransferInternal,
}

impl<'a, R> ROwnedH2BodyEncodeTransfer<'a, R> {
    pub fn new(
        reader: R,
        send_stream: &'a mut SendStream<Bytes>,
        copy_config: &LimitedCopyConfig,
    ) -> Self {
        ROwnedH2BodyEncodeTransfer {
            reader,
            send_stream,
            internal: H2BodyEncodeTransferInternal::new(copy_config),
        }
    }

    #[inline]
    pub fn is_idle(&self) -> bool {
        self.internal.is_idle()
    }

    #[inline]
    pub fn is_active(&self) -> bool {
        self.internal.is_active()
    }

    pub fn reset_active(&mut self) {
        self.internal.reset_active()
    }

    pub fn no_cached_data(&self) -> bool {
        self.internal.no_cached_data()
    }

    pub fn into_io(self) -> (R, &'a mut SendStream<Bytes>) {
        (self.reader, self.send_stream)
    }
}

impl<'a, R> Future for ROwnedH2BodyEncodeTransfer<'a, R>
where
    R: AsyncRead + Unpin,
{
    type Output = Result<(), H2StreamBodyEncodeTransferError>;

    fn poll(mut self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Self::Output> {
        let me = &mut *self;

        me.internal
            .poll_encode(cx, Pin::new(&mut me.reader), me.send_stream)
    }
}
