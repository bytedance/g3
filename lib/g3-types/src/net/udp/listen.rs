/*
 * Copyright 2023 ByteDance and/or its affiliates.
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

use std::net::{IpAddr, Ipv6Addr, SocketAddr};

use anyhow::anyhow;
use num_traits::ToPrimitive;

use crate::net::{SocketBufferConfig, UdpMiscSockOpts};

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct UdpListenConfig {
    address: SocketAddr,
    ipv6only: bool,
    buf_conf: SocketBufferConfig,
    misc_opts: UdpMiscSockOpts,
    instance: usize,
    scale: usize,
}

impl Default for UdpListenConfig {
    fn default() -> Self {
        UdpListenConfig {
            address: SocketAddr::new(IpAddr::V6(Ipv6Addr::UNSPECIFIED), 0),
            ipv6only: false,
            buf_conf: SocketBufferConfig::default(),
            misc_opts: UdpMiscSockOpts::default(),
            instance: 1,
            scale: 0,
        }
    }
}

impl UdpListenConfig {
    pub fn check(&self) -> anyhow::Result<()> {
        if self.address.port() == 0 {
            return Err(anyhow!("no listen port is set"));
        }

        Ok(())
    }

    #[inline]
    pub fn address(&self) -> SocketAddr {
        self.address
    }

    #[inline]
    pub fn socket_buffer(&self) -> SocketBufferConfig {
        self.buf_conf
    }

    #[inline]
    pub fn socket_misc_opts(&self) -> UdpMiscSockOpts {
        self.misc_opts
    }

    #[inline]
    pub fn is_ipv6only(&self) -> bool {
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

    #[inline]
    pub fn set_ipv6_only(&mut self, ipv6only: bool) {
        self.ipv6only = ipv6only;
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
