/*
 * SPDX-License-Identifier: Apache-2.0
 * Copyright 2025 ByteDance and/or its affiliates.
 */

use std::cell::UnsafeCell;
use std::io::IoSliceMut;
use std::net::{IpAddr, SocketAddr};
use std::time::Duration;

use super::RecvAncillaryData;
use crate::RawSocketAddr;

#[cfg(unix)]
mod unix;
#[cfg(windows)]
mod windows;

pub struct RecvMsgHdr<'a, const C: usize> {
    pub iov: [IoSliceMut<'a>; C],
    pub n_recv: usize,
    c_addr: UnsafeCell<RawSocketAddr>,
    dst_ip: Option<IpAddr>,
    interface_id: Option<u32>,
}

impl<const C: usize> RecvAncillaryData for RecvMsgHdr<'_, C> {
    fn set_recv_interface(&mut self, id: u32) {
        self.interface_id = Some(id);
    }

    fn set_recv_dst_addr(&mut self, addr: IpAddr) {
        self.dst_ip = Some(addr);
    }

    fn set_timestamp(&mut self, _ts: Duration) {}
}

impl<'a, const C: usize> RecvMsgHdr<'a, C> {
    pub fn new(iov: [IoSliceMut<'a>; C]) -> Self {
        RecvMsgHdr {
            iov,
            n_recv: 0,
            c_addr: UnsafeCell::new(RawSocketAddr::default()),
            dst_ip: None,
            interface_id: None,
        }
    }

    pub fn src_addr(&self) -> Option<SocketAddr> {
        let c_addr = unsafe { &*self.c_addr.get() };
        c_addr.to_std()
    }

    #[inline]
    pub fn dst_ip(&self) -> Option<IpAddr> {
        self.dst_ip
    }

    pub fn dst_addr(&self, local_addr: SocketAddr) -> SocketAddr {
        self.dst_ip
            .map(|ip| SocketAddr::new(ip, local_addr.port()))
            .unwrap_or(local_addr)
    }

    #[inline]
    pub fn interface_id(&self) -> Option<u32> {
        self.interface_id
    }
}
