/*
 * Copyright 2024 ByteDance and/or its affiliates.
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

use libc::{c_int, c_void, socklen_t};

unsafe fn setsockopt<T>(fd: c_int, level: c_int, name: c_int, value: T) -> io::Result<()>
where
    T: Copy,
{
    let payload = &value as *const T as *const c_void;
    let ret = libc::setsockopt(fd, level, name, payload, size_of::<T>() as socklen_t);
    if ret == -1 {
        return Err(io::Error::last_os_error());
    }
    Ok(())
}

pub(crate) fn set_bind_address_no_port<T: AsRawFd>(fd: &T, enable: bool) -> io::Result<()> {
    unsafe {
        setsockopt(
            fd.as_raw_fd(),
            libc::IPPROTO_IP,
            libc::IP_BIND_ADDRESS_NO_PORT,
            enable as c_int,
        )?;
        Ok(())
    }
}
