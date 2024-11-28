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

use std::io;
use std::io::IoSlice;
use std::net::SocketAddr;
use std::task::{Context, Poll, ready};

use tokio::io::Interest;
use tokio::net::UdpSocket;

use g3_socket::RawSocket;

use super::UdpSocketExt;

impl UdpSocketExt for UdpSocket {
    fn poll_sendmsg(
        &self,
        cx: &mut Context<'_>,
        iov: &[IoSlice<'_>],
        target: Option<SocketAddr>,
    ) -> Poll<io::Result<usize>> {
        let socket = RawSocket::from(self);

        loop {
            ready!(self.poll_send_ready(cx))?;
            match self.try_io(Interest::WRITABLE, || socket.sendmsg(iov, target)) {
                Ok(res) => return Poll::Ready(Ok(res)),
                Err(e) => {
                    if e.kind() == io::ErrorKind::WouldBlock {
                        continue;
                    }
                    return Poll::Ready(Err(e));
                }
            }
        }
    }

    fn try_sendmsg(&self, iov: &[IoSlice<'_>], target: Option<SocketAddr>) -> io::Result<usize> {
        let socket = RawSocket::from(self);

        self.try_io(Interest::WRITABLE, || socket.sendmsg(iov, target))
    }
}
