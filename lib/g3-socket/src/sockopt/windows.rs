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
use std::os::windows::io::AsRawSocket;

use libc::{c_char, c_int, SOCKET};

// windows_sys::Win32::Networking::WinSock::SOL_SOCKET
const SOL_SOCKET: i32 = 65535i32;

// windows_sys::Win32::Networking::WinSock::SO_REUSE_UNICASTPORT
const SO_REUSE_UNICASTPORT: i32 = 12295i32;

unsafe fn setsockopt<T>(socket: SOCKET, level: c_int, name: c_int, value: T) -> io::Result<()>
where
    T: Copy,
{
    let payload = &value as *const T as *const c_char;
    let ret = libc::setsockopt(socket, level, name, payload, size_of::<T>() as c_int);
    if ret == -1 {
        return Err(io::Error::last_os_error());
    }
    Ok(())
}

pub(crate) fn set_reuse_unicastport<T: AsRawSocket>(socket: &T, enable: bool) -> io::Result<()> {
    unsafe {
        setsockopt(
            // std::os::windows::raw::SOCKET is u64
            // windows_sys::Win32::Networking::WinSock::SOCKET is usize
            socket.as_raw_socket() as SOCKET,
            SOL_SOCKET,
            SO_REUSE_UNICASTPORT,
            enable as c_int,
        )?;
        Ok(())
    }
}
