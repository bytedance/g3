/*
 * SPDX-License-Identifier: Apache-2.0
 * Copyright 2025 ByteDance and/or its affiliates.
 */

use std::mem::MaybeUninit;
use std::os::fd::AsRawFd;
use std::{io, ptr};

use libc::{c_int, socklen_t};

#[cfg(any(target_os = "linux", target_os = "android"))]
mod linux;
#[cfg(any(target_os = "linux", target_os = "android"))]
pub(crate) use linux::{
    get_incoming_cpu, set_bind_address_no_port, set_incoming_cpu, set_ip_transparent_v6,
    set_tcp_quick_ack,
};

#[cfg(target_os = "freebsd")]
mod freebsd;
#[cfg(target_os = "freebsd")]
pub(crate) use freebsd::set_tcp_reuseport_lb_numa_current_domain;

#[cfg(target_os = "solaris")]
mod solaris;
#[cfg(target_os = "solaris")]
pub(crate) use solaris::set_tcp_congestion;

#[cfg(target_os = "illumos")]
mod illumos;
#[cfg(target_os = "illumos")]
pub(crate) use illumos::set_tcp_quick_ack;

unsafe fn setsockopt<T>(fd: c_int, level: c_int, name: c_int, value: T) -> io::Result<()>
where
    T: Copy,
{
    unsafe {
        let ret = libc::setsockopt(
            fd,
            level,
            name,
            ptr::from_ref(&value).cast(),
            size_of::<T>() as socklen_t,
        );
        if ret == -1 {
            return Err(io::Error::last_os_error());
        }
        Ok(())
    }
}

unsafe fn getsockopt<T>(fd: c_int, level: c_int, name: c_int) -> io::Result<T>
where
    T: Copy,
{
    let mut value: MaybeUninit<T> = MaybeUninit::zeroed();
    unsafe {
        let mut len = size_of::<T>() as socklen_t;
        let ret = libc::getsockopt(fd, level, name, value.as_mut_ptr().cast(), &mut len);
        if ret == -1 {
            return Err(io::Error::last_os_error());
        }
        Ok(value.assume_init())
    }
}

#[allow(unused)]
pub(crate) fn ipv6_only<T: AsRawFd>(fd: &T) -> io::Result<bool> {
    unsafe {
        let value = getsockopt::<c_int>(fd.as_raw_fd(), libc::IPPROTO_IPV6, libc::IPV6_V6ONLY)?;
        Ok(value != 0)
    }
}

pub(crate) fn set_tos_v4<T: AsRawFd>(fd: &T, tos: u8) -> io::Result<()> {
    unsafe {
        setsockopt(fd.as_raw_fd(), libc::IPPROTO_IP, libc::IP_TOS, tos as c_int)?;
        Ok(())
    }
}

pub(crate) fn set_tclass_v6<T: AsRawFd>(fd: &T, tclass: u8) -> io::Result<()> {
    unsafe {
        setsockopt(
            fd.as_raw_fd(),
            libc::IPPROTO_IPV6,
            libc::IPV6_TCLASS,
            tclass as c_int, // NOTE: -1 is also allowed on solaris and illumos
        )?;
        Ok(())
    }
}

pub(crate) fn set_recv_pktinfo_v6<T: AsRawFd>(fd: &T, enable: bool) -> io::Result<()> {
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
pub(crate) fn set_recv_pktinfo_v4<T: AsRawFd>(fd: &T, enable: bool) -> io::Result<()> {
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
pub(crate) fn set_recv_pktinfo_v4<T: AsRawFd>(fd: &T, enable: bool) -> io::Result<()> {
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
pub(crate) fn set_recv_pktinfo_v4<T: AsRawFd>(fd: &T, enable: bool) -> io::Result<()> {
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

#[cfg(any(target_os = "linux", target_os = "freebsd", target_os = "illumos"))]
pub(crate) fn set_tcp_congestion<T: AsRawFd>(fd: &T, tcp_ca_name: &[u8]) -> io::Result<()> {
    unsafe {
        let ret = libc::setsockopt(
            fd.as_raw_fd(),
            libc::IPPROTO_TCP,
            libc::TCP_CONGESTION,
            tcp_ca_name.as_ptr().cast(),
            tcp_ca_name.len() as socklen_t,
        );
        if ret == -1 {
            return Err(io::Error::last_os_error());
        }
        Ok(())
    }
}
