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
use std::mem::MaybeUninit;
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

unsafe fn getsockopt<T>(fd: c_int, level: c_int, name: c_int) -> io::Result<T>
where
    T: Copy,
{
    let mut payload: MaybeUninit<T> = MaybeUninit::uninit();
    let mut len = size_of::<T>() as socklen_t;
    let ret = libc::getsockopt(fd, level, name, payload.as_mut_ptr().cast(), &mut len);
    if ret == -1 {
        return Err(io::Error::last_os_error());
    }
    Ok(payload.assume_init())
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

pub(crate) fn set_incoming_cpu<T: AsRawFd>(fd: &T, cpu_id: usize) -> io::Result<()> {
    let cpu_id = i32::try_from(cpu_id)
        .map_err(|_| io::Error::new(io::ErrorKind::InvalidInput, "out of range cpu id"))?;
    unsafe {
        setsockopt(
            fd.as_raw_fd(),
            libc::SOL_SOCKET,
            libc::SO_INCOMING_CPU,
            cpu_id,
        )?;
        Ok(())
    }
}

pub(crate) fn get_incoming_cpu<T: AsRawFd>(fd: &T) -> io::Result<usize> {
    unsafe {
        let cpu_id: c_int = getsockopt(fd.as_raw_fd(), libc::SOL_SOCKET, libc::SO_INCOMING_CPU)?;
        usize::try_from(cpu_id).map_err(|e| io::Error::other(format!("invalid cpu id: {e}")))
    }
}
