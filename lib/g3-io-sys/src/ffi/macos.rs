/*
 * SPDX-License-Identifier: Apache-2.0
 * Copyright 2025 ByteDance and/or its affiliates.
 */

use libc::{c_int, c_uint, c_void, iovec, size_t, socklen_t, ssize_t};

#[repr(C)]
pub struct msghdr_x {
    pub msg_name: *mut c_void,
    pub msg_namelen: socklen_t,
    pub msg_iov: *mut iovec,
    pub msg_iovlen: c_int,
    pub msg_control: *mut c_void,
    pub msg_controllen: socklen_t,
    pub msg_flags: c_int,
    pub msg_datalen: size_t,
}

// https://github.com/apple/darwin-xnu/blob/main/bsd/sys/socket.h
unsafe extern "C" {
    pub fn sendmsg_x(s: c_int, msgp: *mut msghdr_x, cnt: c_uint, flags: c_int) -> ssize_t;
    pub fn recvmsg_x(s: c_int, msgp: *mut msghdr_x, cnt: c_uint, flags: c_int) -> ssize_t;
}
