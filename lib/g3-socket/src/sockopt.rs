/*
 * Copyright 2023 ByteDance and/or its affiliates.
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
use std::mem;

use libc::{c_int, c_void};

unsafe fn setsockopt<T>(fd: c_int, opt: c_int, val: c_int, payload: T) -> io::Result<()>
where
    T: Copy,
{
    let payload = &payload as *const T as *const c_void;
    let ret = libc::setsockopt(
        fd,
        opt,
        val,
        payload,
        mem::size_of::<T>() as libc::socklen_t,
    );
    if ret == -1 {
        return Err(io::Error::last_os_error());
    }
    Ok(())
}

pub(crate) fn set_only_ipv6(fd: c_int, only_ipv6: bool) -> io::Result<()> {
    unsafe {
        setsockopt(
            fd,
            libc::IPPROTO_IPV6,
            libc::IPV6_V6ONLY,
            only_ipv6 as c_int,
        )?;
        Ok(())
    }
}

pub(crate) fn set_bind_address_no_port(fd: c_int, enable: bool) -> io::Result<()> {
    unsafe {
        setsockopt(
            fd,
            libc::IPPROTO_IP,
            libc::IP_BIND_ADDRESS_NO_PORT,
            enable as c_int,
        )?;
        Ok(())
    }
}
