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

const DEFAULT_LISTEN_BACKLOG: u32 = 4096;
const MINIMAL_LISTEN_BACKLOG: u32 = 8;

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct TcpListenConfig {
    address: SocketAddr,
    ipv6only: bool,
    backlog: u32,
    instance: usize,
    scale: usize,
}

impl Default for TcpListenConfig {
    fn default() -> Self {
        TcpListenConfig {
            address: SocketAddr::new(IpAddr::V6(Ipv6Addr::UNSPECIFIED), 0),
            ipv6only: false,
            backlog: DEFAULT_LISTEN_BACKLOG,
            instance: 1,
            scale: 0,
        }
    }
}

impl TcpListenConfig {
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
    pub fn is_ipv6only(&self) -> bool {
        self.ipv6only
    }

    #[inline]
    pub fn backlog(&self) -> u32 {
        self.backlog
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
    pub fn set_port(&mut self, port: u16) {
        self.address.set_port(port);
    }

    #[inline]
    pub fn set_ipv6_only(&mut self, ipv6only: bool) {
        self.ipv6only = ipv6only;
    }

    #[inline]
    pub fn set_backlog(&mut self, backlog: u32) {
        if backlog >= MINIMAL_LISTEN_BACKLOG {
            self.backlog = backlog;
        }
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
