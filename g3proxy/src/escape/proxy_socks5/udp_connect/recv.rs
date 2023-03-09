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

use std::io;
use std::task::{ready, Context, Poll};

use futures_util::FutureExt;
use tokio::sync::oneshot;

use g3_io_ext::{AsyncUdpRecv, UdpCopyRemoteError, UdpCopyRemoteRecv};
use g3_socks::v5::UdpInput;

pub(super) struct ProxySocks5UdpConnectRemoteRecv<T> {
    inner: T,
    tcp_close_receiver: oneshot::Receiver<Option<io::Error>>,
}

impl<T> ProxySocks5UdpConnectRemoteRecv<T>
where
    T: AsyncUdpRecv,
{
    pub(super) fn new(recv: T, tcp_close_receiver: oneshot::Receiver<Option<io::Error>>) -> Self {
        ProxySocks5UdpConnectRemoteRecv {
            inner: recv,
            tcp_close_receiver,
        }
    }

    fn poll_recv(
        &mut self,
        cx: &mut Context<'_>,
        buf: &mut [u8],
    ) -> Poll<Result<(usize, usize), UdpCopyRemoteError>> {
        let nr = ready!(self.inner.poll_recv(cx, buf)).map_err(UdpCopyRemoteError::RecvFailed)?;

        let (off, _upstream) = UdpInput::parse_header(buf)
            .map_err(|e| UdpCopyRemoteError::InvalidPacket(e.to_string()))?;
        Poll::Ready(Ok((off, nr)))
    }
}

impl<T> UdpCopyRemoteRecv for ProxySocks5UdpConnectRemoteRecv<T>
where
    T: AsyncUdpRecv,
{
    fn buf_reserve_length(&self) -> usize {
        256 + 4 + 2
    }

    fn poll_recv_packet(
        &mut self,
        cx: &mut Context<'_>,
        buf: &mut [u8],
    ) -> Poll<Result<(usize, usize), UdpCopyRemoteError>> {
        match self.tcp_close_receiver.poll_unpin(cx) {
            Poll::Pending => {}
            Poll::Ready(Ok(None)) => {
                return Poll::Ready(Err(UdpCopyRemoteError::RemoteSessionClosed));
            }
            Poll::Ready(Ok(Some(e))) => {
                return Poll::Ready(Err(UdpCopyRemoteError::RemoteSessionError(e)));
            }
            Poll::Ready(Err(_)) => {
                return Poll::Ready(Err(UdpCopyRemoteError::InternalServerError(
                    "tcp close wait channel closed unexpected",
                )));
            }
        }
        self.poll_recv(cx, buf)
    }
}
