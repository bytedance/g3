/*
 * SPDX-License-Identifier: Apache-2.0
 * Copyright 2025 ByteDance and/or its affiliates.
 */

use std::net::{Ipv4Addr, Ipv6Addr, SocketAddr, SocketAddrV4, SocketAddrV6};

use windows_sys::Win32::Networking::WinSock;

#[derive(Default)]
#[repr(align(8))]
pub(crate) struct RawSocketAddr {
    buf: [u8; size_of::<WinSock::SOCKADDR_IN6>()],
}

impl RawSocketAddr {
    pub(crate) unsafe fn get_ptr_and_size(&mut self) -> (*mut WinSock::SOCKADDR, i32) {
        unsafe {
            let p = &*(self.buf.as_ptr() as *mut WinSock::SOCKADDR);

            let size = match p.sa_family {
                WinSock::AF_INET => size_of::<WinSock::SOCKADDR_IN>(),
                WinSock::AF_INET6 => size_of::<WinSock::SOCKADDR_IN6>(),
                _ => self.buf.len(),
            };

            (self.buf.as_mut_ptr() as _, size as i32)
        }
    }

    pub(crate) fn to_std(&self) -> Option<SocketAddr> {
        let p = unsafe { &*(self.buf.as_ptr() as *mut WinSock::SOCKADDR) };

        match p.sa_family {
            WinSock::AF_INET => {
                let v4 = unsafe { &*(self.buf.as_ptr() as *const WinSock::SOCKADDR_IN) };
                Some(SocketAddr::V4(SocketAddrV4::new(
                    Ipv4Addr::from(u32::from_be(unsafe { v4.sin_addr.S_un.S_addr })),
                    u16::from_be(v4.sin_port),
                )))
            }
            WinSock::AF_INET6 => {
                let v6 = unsafe { &*(self.buf.as_ptr() as *const WinSock::SOCKADDR_IN6) };
                Some(SocketAddr::V6(SocketAddrV6::new(
                    Ipv6Addr::from(unsafe { v6.sin6_addr.u.Byte }),
                    u16::from_be(v6.sin6_port),
                    u32::from_be(v6.sin6_flowinfo),
                    unsafe { v6.Anonymous.sin6_scope_id },
                )))
            }
            _ => None,
        }
    }

    pub(crate) fn set_std(&mut self, addr: SocketAddr) {
        match addr {
            SocketAddr::V4(v4) => {
                let a4 = unsafe { &mut *(self.buf.as_mut_ptr() as *mut WinSock::SOCKADDR_IN) };
                a4.sin_family = WinSock::AF_INET;
                a4.sin_port = u16::to_be(addr.port());
                a4.sin_addr.S_un.S_addr = u32::to_be(v4.ip().to_bits());
            }
            SocketAddr::V6(v6) => {
                let a6 = unsafe { &mut *(self.buf.as_mut_ptr() as *mut WinSock::SOCKADDR_IN6) };
                a6.sin6_family = WinSock::AF_INET6;
                a6.sin6_port = u16::to_be(addr.port());
                a6.sin6_addr.u.Byte = v6.ip().octets();
                a6.sin6_flowinfo = u32::to_be(v6.flowinfo());
                a6.Anonymous.sin6_scope_id = v6.scope_id();
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
