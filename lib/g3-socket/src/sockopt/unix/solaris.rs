/*
 * SPDX-License-Identifier: Apache-2.0
 * Copyright 2025 ByteDance and/or its affiliates.
 */

use std::io;
use std::os::fd::AsRawFd;

use libc::socklen_t;

const TCP_CONGESTION: libc::c_int = 0x23;

#[cfg(target_os = "solaris")]
pub(crate) fn set_tcp_congestion<T: AsRawFd>(fd: &T, tcp_ca_name: &[u8]) -> io::Result<()> {
    unsafe {
        let ret = libc::setsockopt(
            fd.as_raw_fd(),
            libc::IPPROTO_TCP,
            TCP_CONGESTION,
            tcp_ca_name.as_ptr().cast(),
            tcp_ca_name.len() as socklen_t,
        );
        if ret == -1 {
            return Err(io::Error::last_os_error());
        }
        Ok(())
    }
}
