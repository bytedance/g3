/*
 * SPDX-License-Identifier: Apache-2.0
 * Copyright 2025 ByteDance and/or its affiliates.
 */

use std::io;
use std::os::unix::io::AsRawFd;

pub(crate) fn set_tcp_reuseport_lb_numa_current_domain<T: AsRawFd>(fd: &T) -> io::Result<()> {
    const TCP_REUSPORT_LB_NUMA_CURDOM: i32 = -1;

    unsafe {
        super::setsockopt(
            fd.as_raw_fd(),
            libc::IPPROTO_TCP,
            libc::TCP_REUSPORT_LB_NUMA,
            TCP_REUSPORT_LB_NUMA_CURDOM,
        )?;
        Ok(())
    }
}
