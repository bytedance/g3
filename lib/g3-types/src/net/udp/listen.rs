/*
 * SPDX-License-Identifier: Apache-2.0
 * Copyright 2023-2025 ByteDance and/or its affiliates.
 */

use std::net::{IpAddr, Ipv6Addr, SocketAddr};

use anyhow::anyhow;
use num_traits::ToPrimitive;

#[cfg(any(
    target_os = "linux",
    target_os = "android",
    target_os = "macos",
    target_os = "illumos",
    target_os = "solaris"
))]
use crate::net::Interface;
use crate::net::{SocketBufferConfig, UdpMiscSockOpts};

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct UdpListenConfig {
    address: SocketAddr,
    #[cfg(any(
        target_os = "linux",
        target_os = "android",
        target_os = "macos",
        target_os = "illumos",
        target_os = "solaris"
    ))]
    interface: Option<Interface>,
    #[cfg(not(target_os = "openbsd"))]
    ipv6only: Option<bool>,
    buf_conf: SocketBufferConfig,
    misc_opts: UdpMiscSockOpts,
    instance: usize,
    scale: usize,
}

impl Default for UdpListenConfig {
    fn default() -> Self {
        UdpListenConfig::new(SocketAddr::new(IpAddr::V6(Ipv6Addr::UNSPECIFIED), 0))
    }
}

impl UdpListenConfig {
    pub fn new(address: SocketAddr) -> Self {
        UdpListenConfig {
            address,
            #[cfg(any(
                target_os = "linux",
                target_os = "android",
                target_os = "macos",
                target_os = "illumos",
                target_os = "solaris"
            ))]
            interface: None,
            #[cfg(not(target_os = "openbsd"))]
            ipv6only: None,
            buf_conf: SocketBufferConfig::default(),
            misc_opts: UdpMiscSockOpts::default(),
            instance: 1,
            scale: 0,
        }
    }

    pub fn check(&mut self) -> anyhow::Result<()> {
        if self.address.port() == 0 {
            return Err(anyhow!("no listen port is set"));
        }
        #[cfg(not(target_os = "openbsd"))]
        match self.address.ip() {
            IpAddr::V4(_) => self.ipv6only = None,
            IpAddr::V6(v6) => {
                if !v6.is_unspecified() {
                    self.ipv6only = None;
                }
            }
        }

        Ok(())
    }

    #[inline]
    pub fn address(&self) -> SocketAddr {
        self.address
    }

    #[cfg(any(
        target_os = "linux",
        target_os = "android",
        target_os = "macos",
        target_os = "illumos",
        target_os = "solaris"
    ))]
    #[inline]
    pub fn interface(&self) -> Option<&Interface> {
        self.interface.as_ref()
    }

    #[inline]
    pub fn socket_buffer(&self) -> SocketBufferConfig {
        self.buf_conf
    }

    #[inline]
    pub fn socket_misc_opts(&self) -> UdpMiscSockOpts {
        self.misc_opts
    }

    #[cfg(not(target_os = "openbsd"))]
    #[inline]
    pub fn is_ipv6only(&self) -> Option<bool> {
        self.ipv6only
    }

    #[inline]
    pub fn instance(&self) -> usize {
        self.instance.max(self.scale)
    }

    #[inline]
    pub fn set_socket_address(&mut self, addr: SocketAddr) {
        self.address = addr;
    }

    #[cfg(any(
        target_os = "linux",
        target_os = "android",
        target_os = "macos",
        target_os = "illumos",
        target_os = "solaris"
    ))]
    #[inline]
    pub fn set_interface(&mut self, interface: Interface) {
        self.interface = Some(interface);
    }

    #[inline]
    pub fn set_socket_buffer(&mut self, buf_conf: SocketBufferConfig) {
        self.buf_conf = buf_conf;
    }

    #[inline]
    pub fn set_socket_misc_opts(&mut self, misc_opts: UdpMiscSockOpts) {
        self.misc_opts = misc_opts;
    }

    #[inline]
    pub fn set_port(&mut self, port: u16) {
        self.address.set_port(port);
    }

    #[cfg(not(target_os = "openbsd"))]
    #[inline]
    pub fn set_ipv6_only(&mut self, ipv6only: bool) {
        self.ipv6only = Some(ipv6only);
    }

    pub fn set_instance(&mut self, instance: usize) {
        if instance == 0 {
            self.instance = 1;
        } else {
            self.instance = instance;
        }
    }

    pub fn set_scale(&mut self, scale: f64) -> anyhow::Result<()> {
        if let Ok(p) = std::thread::available_parallelism() {
            let v = (p.get() as f64) * scale;
            self.scale = v
                .round()
                .to_usize()
                .ok_or(anyhow!("out of range result: {v}"))?;
        }
        Ok(())
    }

    pub fn set_fraction_scale(&mut self, numerator: usize, denominator: usize) {
        if let Ok(p) = std::thread::available_parallelism() {
            let v = p.get() * numerator / denominator;
            self.scale = v;
        }
    }
}
