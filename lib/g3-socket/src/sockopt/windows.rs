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

use windows_sys::Win32::Networking::WinSock;

unsafe fn setsockopt<T>(socket: WinSock::SOCKET, level: i32, name: i32, value: T) -> io::Result<()>
where
    T: Copy,
{
    unsafe {
        let payload = &value as *const T as *const u8;
        let ret = WinSock::setsockopt(socket, level, name, payload, size_of::<T>() as i32);
        if ret == WinSock::SOCKET_ERROR {
            return Err(io::Error::last_os_error());
        }
        Ok(())
    }
}

pub(crate) fn set_reuse_unicastport<T: AsRawSocket>(socket: &T, enable: bool) -> io::Result<()> {
    unsafe {
        setsockopt(
            // std::os::windows::raw::SOCKET is u64
            // windows_sys::Win32::Networking::WinSock::SOCKET is usize
            socket.as_raw_socket() as _,
            WinSock::SOL_SOCKET,
            WinSock::SO_REUSE_UNICASTPORT,
            enable as i32,
        )?;
        Ok(())
    }
}
