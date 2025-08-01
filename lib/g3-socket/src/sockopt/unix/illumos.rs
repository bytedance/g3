/*
 * SPDX-License-Identifier: Apache-2.0
 * Copyright 2025 ByteDance and/or its affiliates.
 */

use std::io;
use std::os::fd::AsRawFd;

use libc::c_int;

pub(crate) fn set_tcp_quick_ack<T: AsRawFd>(fd: &T, enable: bool) -> io::Result<()> {
    unsafe {
        super::setsockopt(fd.as_raw_fd(), libc::IPPROTO_TCP, 0x26, enable as c_int)?;
        Ok(())
    }
}
