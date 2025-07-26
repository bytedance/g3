/*
 * SPDX-License-Identifier: Apache-2.0
 * Copyright 2024-2025 ByteDance and/or its affiliates.
 */

use std::io;
use std::mem::MaybeUninit;
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

unsafe fn getsockopt<T>(socket: WinSock::SOCKET, level: i32, name: i32) -> io::Result<T>
where
    T: Copy,
{
    let mut value: MaybeUninit<T> = MaybeUninit::zeroed();
    unsafe {
        let mut len = size_of::<T>() as i32;
        let ret = WinSock::getsockopt(socket, level, name, value.as_mut_ptr().cast(), &mut len);
        if ret == WinSock::SOCKET_ERROR {
            return Err(io::Error::last_os_error());
        }
        Ok(value.assume_init())
    }
}

pub(crate) fn ipv6_only<T: AsRawSocket>(socket: &T) -> io::Result<bool> {
    unsafe {
        let value = getsockopt::<u32>(
            socket.as_raw_socket() as _,
            WinSock::IPPROTO_IPV6,
            WinSock::IPV6_V6ONLY,
        )?;
        Ok(value != 0)
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
            enable as u32,
        )?;
        Ok(())
    }
}

pub(crate) fn set_recv_ip_pktinfo<T: AsRawSocket>(socket: &T, enable: bool) -> io::Result<()> {
    unsafe {
        setsockopt(
            // std::os::windows::raw::SOCKET is u64
            // windows_sys::Win32::Networking::WinSock::SOCKET is usize
            socket.as_raw_socket() as _,
            WinSock::IPPROTO_IP, // not same as IPPROTO_IPV4
            WinSock::IP_PKTINFO,
            enable as u32,
        )?;
        Ok(())
    }
}

pub(crate) fn set_recv_ipv6_pktinfo<T: AsRawSocket>(socket: &T, enable: bool) -> io::Result<()> {
    unsafe {
        setsockopt(
            // std::os::windows::raw::SOCKET is u64
            // windows_sys::Win32::Networking::WinSock::SOCKET is usize
            socket.as_raw_socket() as _,
            WinSock::IPPROTO_IPV6,
            WinSock::IPV6_PKTINFO,
            enable as u32,
        )?;
        Ok(())
    }
}
