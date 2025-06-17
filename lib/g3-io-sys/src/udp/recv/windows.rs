/*
 * SPDX-License-Identifier: Apache-2.0
 * Copyright 2025 ByteDance and/or its affiliates.
 */

use windows_sys::Win32::Networking::WinSock;

use super::RecvMsgHdr;
use crate::udp::RecvAncillaryBuffer;

impl<const C: usize> RecvMsgHdr<'_, C> {
    /// # Safety
    ///
    /// `self` should not be dropped before the returned value
    pub unsafe fn to_msghdr(&self, control_buf: &mut RecvAncillaryBuffer) -> WinSock::WSAMSG {
        let control_buf = control_buf.as_bytes();
        unsafe {
            let c_addr = &mut *self.c_addr.get();
            let (name, namelen) = c_addr.get_ptr_and_size();

            WinSock::WSAMSG {
                name,
                namelen,
                lpBuffers: self.iov.as_ptr() as _,
                dwBufferCount: C as _,
                Control: WinSock::WSABUF {
                    len: control_buf.len() as _,
                    buf: control_buf.as_ptr() as _,
                },
                dwFlags: 0,
            }
        }
    }
}
