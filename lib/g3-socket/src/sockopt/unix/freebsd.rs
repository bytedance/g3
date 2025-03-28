/*
 * Copyright 2025 ByteDance and/or its affiliates.
 *
 * Licensed under the Apache License, Version 2.0 (the "License");
 * you may not use this file except in compliance with the License.
 * You may obtain a copy of the License at
 *
 *     http://www.apache.org/licenses/LICENSE-2.0
 *
 * Unless required by applicable law or agreed to in writing, software
 * distributed under the License is distributed on an "AS IS" BASIS,
 * WITHOUT WARRANTIES OR CONDITIONS OF ANY KIND, either express or implied.
 * See the License for the specific language governing permissions and
 * limitations under the License.
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
