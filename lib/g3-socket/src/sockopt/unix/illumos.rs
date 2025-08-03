/*
 * SPDX-License-Identifier: Apache-2.0
 * Copyright 2025 ByteDance and/or its affiliates.
 */

use std::io;
use std::os::fd::AsRawFd;

use libc::c_int;

const TCP_QUICKACK: c_int = 0x26;

pub(crate) fn set_tcp_quick_ack<T: AsRawFd>(fd: &T, enable: bool) -> io::Result<()> {
    unsafe {
        super::setsockopt(
            fd.as_raw_fd(),
            libc::IPPROTO_TCP,
            TCP_QUICKACK,
            enable as c_int,
        )?;
        Ok(())
    }
}
