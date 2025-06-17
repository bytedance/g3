/*
 * SPDX-License-Identifier: Apache-2.0
 * Copyright 2025 ByteDance and/or its affiliates.
 */

use std::mem;

use super::RecvMsgHdr;
use crate::udp::RecvAncillaryBuffer;

impl<const C: usize> RecvMsgHdr<'_, C> {
    /// # Safety
    ///
    /// `self` should not be dropped before the returned value
    pub unsafe fn to_msghdr(&self, control_buf: &mut RecvAncillaryBuffer) -> libc::msghdr {
        let control_buf = control_buf.as_bytes();
        unsafe {
            let c_addr = &mut *self.c_addr.get();
            let (c_addr, c_addr_len) = c_addr.get_ptr_and_size();

            let mut h = mem::zeroed::<libc::msghdr>();
            h.msg_name = c_addr as _;
            h.msg_namelen = c_addr_len as _;
            h.msg_iov = self.iov.as_ptr() as _;
            h.msg_iovlen = C as _;
            h.msg_control = control_buf.as_ptr() as _;
            h.msg_controllen = control_buf.len() as _;
            h
        }
    }

    /// # Safety
    ///
    /// `self` should not be dropped before the returned value
    #[cfg(target_os = "macos")]
    pub unsafe fn to_msghdr_x(
        &self,
        control_buf: &mut RecvAncillaryBuffer,
    ) -> crate::ffi::msghdr_x {
        let control_buf = control_buf.as_bytes();
        unsafe {
            let c_addr = &mut *self.c_addr.get();
            let (c_addr, c_addr_len) = c_addr.get_ptr_and_size();

            let mut h = mem::zeroed::<crate::ffi::msghdr_x>();
            h.msg_name = c_addr as _;
            h.msg_namelen = c_addr_len as _;
            h.msg_iov = self.iov.as_ptr() as _;
            h.msg_iovlen = C as _;
            h.msg_control = control_buf.as_ptr() as _;
            h.msg_controllen = control_buf.len() as _;
            h
        }
    }
}
