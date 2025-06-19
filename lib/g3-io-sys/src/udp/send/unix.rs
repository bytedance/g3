/*
 * SPDX-License-Identifier: Apache-2.0
 * Copyright 2025 ByteDance and/or its affiliates.
 */

use std::os::fd::AsRawFd;
use std::{io, mem, ptr};

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
    #[cfg(any(
        target_os = "linux",
        target_os = "android",
        target_os = "freebsd",
        target_os = "netbsd",
        target_os = "openbsd",
        target_os = "solaris",
    ))]
    pub unsafe fn to_mmsghdr(&self) -> libc::mmsghdr {
        libc::mmsghdr {
            msg_hdr: unsafe { self.to_msghdr() },
            msg_len: 0,
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

pub fn sendmsg<T: AsRawFd>(fd: &T, msghdr: &mut libc::msghdr) -> io::Result<usize> {
    let r = unsafe { libc::sendmsg(fd.as_raw_fd(), ptr::from_mut(msghdr), libc::MSG_NOSIGNAL) };
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
pub fn sendmmsg<T: AsRawFd>(fd: &T, msgvec: &mut [libc::mmsghdr]) -> io::Result<usize> {
    let r = unsafe {
        libc::sendmmsg(
            fd.as_raw_fd(),
            msgvec.as_mut_ptr(),
            msgvec.len() as _,
            libc::MSG_NOSIGNAL as _,
        )
    };
    if r < 0 {
        Err(io::Error::last_os_error())
    } else {
        Ok(r as usize)
    }
}

#[cfg(target_os = "macos")]
pub fn sendmsg_x<T: AsRawFd>(fd: &T, msgvec: &mut [crate::ffi::msghdr_x]) -> io::Result<usize> {
    let r = unsafe {
        crate::ffi::sendmsg_x(
            fd.as_raw_fd(),
            msgvec.as_mut_ptr(),
            msgvec.len() as _,
            libc::MSG_NOSIGNAL,
        )
    };
    if r < 0 {
        Err(io::Error::last_os_error())
    } else {
        Ok(r as usize)
    }
}
