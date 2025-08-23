/*
 * SPDX-License-Identifier: Apache-2.0
 * Copyright 2025 ByteDance and/or its affiliates.
 */

use std::os::windows::io::AsRawSocket;
use std::{io, ptr};

use windows_sys::Win32::Networking::WinSock;

use super::SendMsgHdr;

impl<'a, const C: usize> SendMsgHdr<'a, C> {
    /// # Safety
    ///
    /// `self` should not be dropped before the returned value
    pub unsafe fn to_msghdr(&self) -> WinSock::WSAMSG {
        unsafe {
            let (name, namelen) = match &self.c_addr {
                Some(v) => {
                    let c = &mut *v.get();
                    c.get_ptr_and_size()
                }
                None => (ptr::null_mut(), 0),
            };

            WinSock::WSAMSG {
                name,
                namelen,
                lpBuffers: self.iov.as_ptr() as _,
                dwBufferCount: C as _,
                Control: WinSock::WSABUF {
                    len: 0,
                    buf: ptr::null_mut(),
                },
                dwFlags: 0,
            }
        }
    }
}

pub fn sendmsg<T: AsRawSocket>(socket: &T, msghdr: &mut WinSock::WSAMSG) -> io::Result<usize> {
    let mut n_sent = 0u32;
    let r = unsafe {
        WinSock::WSASendMsg(
            socket.as_raw_socket() as _,
            ptr::from_mut(msghdr),
            0,
            &mut n_sent,
            ptr::null_mut(),
            None,
        )
    };
    if r != 0 {
        Err(io::Error::last_os_error())
    } else {
        Ok(n_sent as usize)
    }
}
