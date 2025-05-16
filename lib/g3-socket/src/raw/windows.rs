/*
 * SPDX-License-Identifier: Apache-2.0
 * Copyright 2024-2025 ByteDance and/or its affiliates.
 */

use std::io::{self, IoSlice};
use std::net::SocketAddr;
use std::os::windows::io::{AsRawSocket, FromRawSocket, IntoRawSocket};

use socket2::{MsgHdr, SockAddr, Socket};

use super::RawSocket;

impl RawSocket {
    pub fn sendmsg(&self, iov: &[IoSlice<'_>], target: Option<SocketAddr>) -> io::Result<usize> {
        let msg_hdr = MsgHdr::new().with_buffers(iov);
        let target = target.map(SockAddr::from);
        let msg_hdr = if let Some(addr) = &target {
            msg_hdr.with_addr(addr)
        } else {
            msg_hdr
        };

        let socket = self.get_inner()?;
        socket.sendmsg(&msg_hdr, 0)
    }
}

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
