/*
 * SPDX-License-Identifier: Apache-2.0
 * Copyright 2025 ByteDance and/or its affiliates.
 */

use std::net::{Ipv4Addr, Ipv6Addr, SocketAddr, SocketAddrV4, SocketAddrV6};

use libc::{c_int, sockaddr, sockaddr_in, sockaddr_in6};

#[derive(Default)]
#[repr(align(8))]
pub(crate) struct RawSocketAddr {
    buf: [u8; size_of::<sockaddr_in6>()],
}

impl RawSocketAddr {
    fn sa_family(&self) -> c_int {
        let p = unsafe { self.buf.as_ptr().cast::<sockaddr>().as_ref().unwrap() };
        p.sa_family as c_int
    }

    pub(crate) unsafe fn get_ptr_and_size(&mut self) -> (*mut libc::c_void, usize) {
        let size = match self.sa_family() {
            libc::AF_INET => size_of::<sockaddr_in>(),
            libc::AF_INET6 => size_of::<sockaddr_in6>(),
            _ => self.buf.len(),
        };

        (self.buf.as_mut_ptr() as _, size)
    }

    pub(crate) fn to_std(&self) -> Option<SocketAddr> {
        match self.sa_family() {
            libc::AF_INET => {
                let v4 = unsafe { self.buf.as_ptr().cast::<sockaddr_in>().as_ref().unwrap() };
                Some(SocketAddr::V4(SocketAddrV4::new(
                    Ipv4Addr::from(u32::from_be(v4.sin_addr.s_addr)),
                    u16::from_be(v4.sin_port),
                )))
            }
            libc::AF_INET6 => {
                let v6 = unsafe { self.buf.as_ptr().cast::<sockaddr_in6>().as_ref().unwrap() };
                Some(SocketAddr::V6(SocketAddrV6::new(
                    Ipv6Addr::from(v6.sin6_addr.s6_addr),
                    u16::from_be(v6.sin6_port),
                    u32::from_be(v6.sin6_flowinfo),
                    v6.sin6_scope_id,
                )))
            }
            _ => None,
        }
    }

    pub(crate) fn set_std(&mut self, addr: SocketAddr) {
        match addr {
            SocketAddr::V4(v4) => {
                let a4 = unsafe {
                    self.buf
                        .as_mut_ptr()
                        .cast::<sockaddr_in>()
                        .as_mut()
                        .unwrap()
                };
                a4.sin_family = libc::AF_INET as _;
                a4.sin_port = u16::to_be(addr.port());
                a4.sin_addr = libc::in_addr {
                    s_addr: u32::from_ne_bytes(v4.ip().octets()),
                };
            }
            SocketAddr::V6(v6) => {
                let a6 = unsafe {
                    self.buf
                        .as_mut_ptr()
                        .cast::<sockaddr_in6>()
                        .as_mut()
                        .unwrap()
                };
                a6.sin6_family = libc::AF_INET6 as _;
                a6.sin6_port = u16::to_be(addr.port());
                a6.sin6_addr = libc::in6_addr {
                    s6_addr: v6.ip().octets(),
                };
                a6.sin6_flowinfo = u32::to_be(v6.flowinfo());
                a6.sin6_scope_id = v6.scope_id();
            }
        }
    }
}

impl From<SocketAddr> for RawSocketAddr {
    fn from(value: SocketAddr) -> Self {
        let mut v = RawSocketAddr::default();
        v.set_std(value);
        v
    }
}
