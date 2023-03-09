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
use std::pin::Pin;
use std::task::{ready, Context, Poll};

use bytes::{Buf, Bytes};
use h2::{FlowControl, RecvStream, SendStream};

use super::H2StreamBodyTransferError;

pub struct H2BodyTransfer {
    yield_size: usize,
    recv_stream: RecvStream,
    recv_flow_control: FlowControl,
    send_stream: SendStream<Bytes>,
    send_chunk: Option<Bytes>,
    handle_trailers: bool,
    active: bool,
}

impl H2BodyTransfer {
    pub fn new(
        mut recv_stream: RecvStream,
        send_stream: SendStream<Bytes>,
        yield_size: usize,
    ) -> Self {
        let recv_flow_control = recv_stream.flow_control().clone();
        H2BodyTransfer {
            yield_size,
            recv_stream,
            recv_flow_control,
            send_stream,
            send_chunk: None,
            handle_trailers: false,
            active: false,
        }
    }

    #[inline]
    pub fn is_idle(&self) -> bool {
        !self.active
    }

    pub fn reset_active(&mut self) {
        self.active = false;
    }

    pub fn no_cached_data(&self) -> bool {
        self.send_chunk.is_none() && !self.handle_trailers
    }

    fn poll_transfer_trailers(
        &mut self,
        cx: &mut Context<'_>,
    ) -> Poll<Result<(), H2StreamBodyTransferError>> {
        match ready!(self.recv_stream.poll_trailers(cx)) {
            Ok(Some(trailers)) => {
                self.send_stream
                    .send_trailers(trailers)
                    .map_err(H2StreamBodyTransferError::SendTrailersFailed)?;
                Poll::Ready(Ok(()))
            }
            Ok(None) => {
                self.send_stream
                    .send_data(Bytes::new(), true)
                    .map_err(H2StreamBodyTransferError::GracefulCloseError)?;
                Poll::Ready(Ok(()))
            }
            Err(e) => Poll::Ready(Err(H2StreamBodyTransferError::RecvTrailersFailed(e))),
        }
    }

    fn poll_transfer(
        &mut self,
        cx: &mut Context<'_>,
    ) -> Poll<Result<(), H2StreamBodyTransferError>> {
        if self.handle_trailers {
            return self.poll_transfer_trailers(cx);
        }

        let mut copy_this_round = 0usize;

        loop {
            if let Some(mut chunk) = self.send_chunk.take() {
                match self.send_stream.poll_capacity(cx) {
                    Poll::Ready(Some(Ok(n))) => {
                        self.active = true;
                        let to_send = chunk.split_to(n);
                        self.send_stream
                            .send_data(to_send, false)
                            .map_err(H2StreamBodyTransferError::SendDataFailed)?;
                        self.recv_flow_control
                            .release_capacity(n)
                            .map_err(H2StreamBodyTransferError::ReleaseRecvCapacityFailed)?;
                        if chunk.has_remaining() {
                            self.send_chunk = Some(chunk);
                        }

                        copy_this_round += n;
                        if copy_this_round >= self.yield_size {
                            cx.waker().wake_by_ref();
                            return Poll::Pending;
                        }
                    }
                    Poll::Ready(Some(Err(e))) => {
                        self.send_chunk = Some(chunk);
                        return Poll::Ready(Err(
                            H2StreamBodyTransferError::WaitSendCapacityFailed(e),
                        ));
                    }
                    Poll::Ready(None) => {
                        // only possible if the reserve capacity is 0 or not set
                        unreachable!()
                    }
                    Poll::Pending => {
                        self.send_chunk = Some(chunk);
                        return Poll::Pending;
                    }
                }
            } else {
                match ready!(self.recv_stream.poll_data(cx)) {
                    Some(Ok(chunk)) => {
                        self.active = true;
                        if chunk.has_remaining() {
                            self.send_stream.reserve_capacity(chunk.len());
                            self.send_chunk = Some(chunk);
                            continue;
                        }
                    }
                    Some(Err(e)) => {
                        return Poll::Ready(Err(H2StreamBodyTransferError::RecvDataFailed(e)));
                    }
                    None => {}
                }

                return if self.recv_stream.is_end_stream() {
                    self.send_stream
                        .send_data(Bytes::new(), true)
                        .map_err(H2StreamBodyTransferError::GracefulCloseError)?;
                    Poll::Ready(Ok(()))
                } else {
                    self.handle_trailers = true;
                    self.poll_transfer_trailers(cx)
                };
            }
        }
    }
}

impl Future for H2BodyTransfer {
    type Output = Result<(), H2StreamBodyTransferError>;

    fn poll(mut self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Self::Output> {
        self.poll_transfer(cx)
    }
}
