/*
 * SPDX-License-Identifier: Apache-2.0
 * Copyright 2025 ByteDance and/or its affiliates.
 */

use std::ptr;

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
