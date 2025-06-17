/*
 * SPDX-License-Identifier: Apache-2.0
 * Copyright 2025 ByteDance and/or its affiliates.
 */

use std::{mem, ptr};

use super::SendMsgHdr;

impl<'a, const C: usize> SendMsgHdr<'a, C> {
    /// # Safety
    ///
    /// `self` should not be dropped before the returned value
    pub unsafe fn to_msghdr(&self) -> libc::msghdr {
        unsafe {
            let (c_addr, c_addr_len) = match &self.c_addr {
                Some(v) => {
                    let c = &mut *v.get();
                    c.get_ptr_and_size()
                }
                None => (ptr::null_mut(), 0),
            };

            let mut h = mem::zeroed::<libc::msghdr>();
            h.msg_name = c_addr as _;
            h.msg_namelen = c_addr_len as _;
            h.msg_iov = self.iov.as_ptr() as _;
            h.msg_iovlen = C as _;
            h
        }
    }

    /// # Safety
    ///
    /// `self` should not be dropped before the returned value
    #[cfg(target_os = "macos")]
    pub unsafe fn to_msghdr_x(&self) -> crate::ffi::msghdr_x {
        unsafe {
            let mut h = mem::zeroed::<crate::ffi::msghdr_x>();
            h.msg_iov = self.iov.as_ptr() as _;
            h.msg_iovlen = C as _;
            h
        }
    }
}
