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

use std::io::{self, IoSlice};
use std::net::SocketAddr;
use std::os::windows::io::{AsRawSocket, FromRawSocket};

use socket2::{MsgHdr, SockAddr, Socket};

use super::RawSocket;

impl RawSocket {
    pub fn sendmsg(&self, iov: &[IoSlice<'_>], target: Option<SocketAddr>) -> io::Result<usize> {
        let msg_hdr = MsgHdr::new().with_buffers(iov);
        let target = target.map(|v| SockAddr::from(v));
        let msg_hdr = if let Some(addr) = &target {
            msg_hdr.with_addr(&addr)
        } else {
            msg_hdr
        };

        let socket = self.get_inner()?;
        socket.sendmsg(&msg_hdr, 0)
    }
}

#[cfg(unix)]
impl Drop for RawSocket {
    fn drop(&mut self) {
        if let Some(s) = self.inner.take() {
            let _ = s.into_raw_socket();
        }
    }
}

impl Clone for RawSocket {
    fn clone(&self) -> Self {
        if let Some(s) = &self.inner {
            Self::from(s)
        } else {
            RawSocket { inner: None }
        }
    }
}

impl<T: AsRawSocket> From<&T> for RawSocket {
    fn from(value: &T) -> Self {
        let socket = unsafe { Socket::from_raw_socket(value.as_raw_socket()) };
        RawSocket {
            inner: Some(socket),
        }
    }
}
