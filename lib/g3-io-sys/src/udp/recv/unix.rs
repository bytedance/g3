/*
 * SPDX-License-Identifier: Apache-2.0
 * Copyright 2025 ByteDance and/or its affiliates.
 */

use std::os::fd::AsRawFd;
use std::{io, mem, ptr};

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
    #[cfg(any(
        target_os = "linux",
        target_os = "android",
        target_os = "freebsd",
        target_os = "netbsd",
        target_os = "openbsd",
        target_os = "solaris",
    ))]
    pub unsafe fn to_mmsghdr(&self, control_buf: &mut RecvAncillaryBuffer) -> libc::mmsghdr {
        libc::mmsghdr {
            msg_hdr: unsafe { self.to_msghdr(control_buf) },
            msg_len: 0,
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

pub fn recvmsg<T: AsRawFd>(fd: &T, msghdr: &mut libc::msghdr) -> io::Result<usize> {
    let r = unsafe { libc::recvmsg(fd.as_raw_fd(), ptr::from_mut(msghdr), libc::MSG_DONTWAIT) };
    if r < 0 {
        Err(io::Error::last_os_error())
    } else {
        Ok(r as usize)
    }
}

#[cfg(any(
    target_os = "linux",
    target_os = "android",
    target_os = "freebsd",
    target_os = "netbsd",
    target_os = "openbsd",
    target_os = "solaris",
))]
pub fn recvmmsg<T: AsRawFd>(fd: &T, msgvec: &mut [libc::mmsghdr]) -> io::Result<usize> {
    let r = unsafe {
        libc::recvmmsg(
            fd.as_raw_fd(),
            msgvec.as_mut_ptr(),
            msgvec.len() as _,
            libc::MSG_DONTWAIT as _,
            ptr::null_mut(),
        )
    };
    if r < 0 {
        Err(io::Error::last_os_error())
    } else {
        Ok(r as usize)
    }
}

#[cfg(target_os = "macos")]
pub fn recvmsg_x<T: AsRawFd>(fd: &T, msgvec: &mut [crate::ffi::msghdr_x]) -> io::Result<usize> {
    let r = unsafe {
        crate::ffi::recvmsg_x(
            fd.as_raw_fd(),
            msgvec.as_mut_ptr(),
            msgvec.len() as _,
            libc::MSG_DONTWAIT,
        )
    };
    if r < 0 {
        Err(io::Error::last_os_error())
    } else {
        Ok(r as usize)
    }
}
