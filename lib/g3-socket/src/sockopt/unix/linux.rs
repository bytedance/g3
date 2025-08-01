/*
 * SPDX-License-Identifier: Apache-2.0
 * Copyright 2024-2025 ByteDance and/or its affiliates.
 */

use std::io;
use std::mem::MaybeUninit;
use std::os::unix::io::AsRawFd;

use libc::{c_int, socklen_t};

unsafe fn getsockopt<T>(fd: c_int, level: c_int, name: c_int) -> io::Result<T>
where
    T: Copy,
{
    unsafe {
        let mut payload: MaybeUninit<T> = MaybeUninit::uninit();
        let mut len = size_of::<T>() as socklen_t;
        let ret = libc::getsockopt(fd, level, name, payload.as_mut_ptr().cast(), &mut len);
        if ret == -1 {
            return Err(io::Error::last_os_error());
        }
        Ok(payload.assume_init())
    }
}

pub(crate) fn set_bind_address_no_port<T: AsRawFd>(fd: &T, enable: bool) -> io::Result<()> {
    unsafe {
        super::setsockopt(
            fd.as_raw_fd(),
            libc::IPPROTO_IP,
            libc::IP_BIND_ADDRESS_NO_PORT,
            enable as c_int,
        )?;
        Ok(())
    }
}

pub(crate) fn set_ip_transparent_v6<T: AsRawFd>(fd: &T, enable: bool) -> io::Result<()> {
    unsafe {
        super::setsockopt(
            fd.as_raw_fd(),
            libc::IPPROTO_IPV6,
            libc::IPV6_TRANSPARENT,
            enable as c_int,
        )?;
        Ok(())
    }
}

pub(crate) fn set_incoming_cpu<T: AsRawFd>(fd: &T, cpu_id: usize) -> io::Result<()> {
    let cpu_id = i32::try_from(cpu_id)
        .map_err(|_| io::Error::new(io::ErrorKind::InvalidInput, "out of range cpu id"))?;
    unsafe {
        super::setsockopt(
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

pub(crate) fn set_tcp_quick_ack<T: AsRawFd>(fd: &T, enable: bool) -> io::Result<()> {
    unsafe {
        super::setsockopt(
            fd.as_raw_fd(),
            libc::IPPROTO_TCP,
            libc::TCP_QUICKACK,
            enable as c_int,
        )?;
        Ok(())
    }
}
