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

use std::io::{self, IoSlice};
use std::net::SocketAddr;
use std::os::fd::AsRawFd;
use std::task::{ready, Context, Poll};

use nix::sys::socket::{sendmsg, MsgFlags, SockaddrStorage};
use tokio::io::Interest;
use tokio::net::UdpSocket;

pub trait UdpSocketExt {
    fn poll_sendmsg(
        &self,
        cx: &mut Context<'_>,
        iov: &[IoSlice<'_>],
        target: Option<SocketAddr>,
    ) -> Poll<io::Result<usize>>;
}

impl UdpSocketExt for UdpSocket {
    fn poll_sendmsg(
        &self,
        cx: &mut Context<'_>,
        iov: &[IoSlice<'_>],
        target: Option<SocketAddr>,
    ) -> Poll<io::Result<usize>> {
        #[cfg(not(target_os = "macos"))]
        let flags: MsgFlags = MsgFlags::MSG_DONTWAIT | MsgFlags::MSG_NOSIGNAL;
        #[cfg(target_os = "macos")]
        let flags: MsgFlags = MsgFlags::MSG_DONTWAIT;

        let raw_fd = self.as_raw_fd();
        let addr = target.map(SockaddrStorage::from);
        loop {
            ready!(self.poll_send_ready(cx))?;
            if let Ok(res) = self.try_io(Interest::WRITABLE, || {
                sendmsg(raw_fd, iov, &[], flags, addr.as_ref()).map_err(io::Error::from)
            }) {
                return Poll::Ready(Ok(res));
            }
        }
    }
}
