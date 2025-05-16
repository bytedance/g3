/*
 * SPDX-License-Identifier: Apache-2.0
 * Copyright 2025 ByteDance and/or its affiliates.
 */

use std::io;
use std::os::fd::AsRawFd;

use libc::{c_int, c_void, socklen_t};

#[cfg(any(target_os = "linux", target_os = "android"))]
mod linux;
#[cfg(any(target_os = "linux", target_os = "android"))]
pub(crate) use linux::{get_incoming_cpu, set_bind_address_no_port, set_incoming_cpu};

#[cfg(target_os = "freebsd")]
mod freebsd;
#[cfg(target_os = "freebsd")]
pub(crate) use freebsd::set_tcp_reuseport_lb_numa_current_domain;

unsafe fn setsockopt<T>(fd: c_int, level: c_int, name: c_int, value: T) -> io::Result<()>
where
    T: Copy,
{
    unsafe {
        let payload = &value as *const T as *const c_void;
        let ret = libc::setsockopt(fd, level, name, payload, size_of::<T>() as socklen_t);
        if ret == -1 {
            return Err(io::Error::last_os_error());
        }
        Ok(())
    }
}

pub(crate) fn set_recv_ipv6_pktinfo<T: AsRawFd>(fd: &T, enable: bool) -> io::Result<()> {
    unsafe {
        setsockopt(
            fd.as_raw_fd(),
            libc::IPPROTO_IPV6,
            libc::IPV6_RECVPKTINFO,
            enable as c_int,
        )?;
        Ok(())
    }
}

#[cfg(any(
    target_os = "linux",
    target_os = "android",
    target_os = "macos",
    target_os = "illumos"
))]
pub(crate) fn set_recv_ip_pktinfo<T: AsRawFd>(fd: &T, enable: bool) -> io::Result<()> {
    unsafe {
        setsockopt(
            fd.as_raw_fd(),
            libc::IPPROTO_IP,
            libc::IP_PKTINFO,
            enable as c_int,
        )?;
        Ok(())
    }
}

#[cfg(any(target_os = "freebsd", target_os = "openbsd", target_os = "dragonfly"))]
pub(crate) fn set_recv_ip_pktinfo<T: AsRawFd>(fd: &T, enable: bool) -> io::Result<()> {
    unsafe {
        setsockopt(
            fd.as_raw_fd(),
            libc::IPPROTO_IP,
            libc::IP_RECVDSTADDR,
            enable as c_int,
        )?;
        setsockopt(
            fd.as_raw_fd(),
            libc::IPPROTO_IP,
            libc::IP_RECVIF,
            enable as c_int,
        )?;
        Ok(())
    }
}

#[cfg(not(any(
    target_os = "linux",
    target_os = "android",
    target_os = "macos",
    target_os = "illumos",
    target_os = "freebsd",
    target_os = "openbsd",
    target_os = "dragonfly"
)))]
pub(crate) fn set_recv_ip_pktinfo<T: AsRawFd>(fd: &T, enable: bool) -> io::Result<()> {
    unsafe {
        setsockopt(
            fd.as_raw_fd(),
            libc::IPPROTO_IP,
            libc::IP_RECVPKTINFO,
            enable as c_int,
        )?;
        Ok(())
    }
}
