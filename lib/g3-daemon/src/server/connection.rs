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

use std::io;
use std::net::{IpAddr, SocketAddr};
use std::os::fd::RawFd;

use g3_io_ext::haproxy::ProxyAddr;
use g3_types::net::TcpMiscSockOpts;

#[derive(Clone, Debug)]
pub struct ClientConnectionInfo {
    client_addr: SocketAddr,
    server_addr: SocketAddr,
    sock_peer_addr: SocketAddr,
    #[allow(unused)]
    sock_local_addr: SocketAddr,
    sock_raw_fd: RawFd,
}

impl ClientConnectionInfo {
    pub fn new(peer_addr: SocketAddr, local_addr: SocketAddr, raw_fd: RawFd) -> Self {
        ClientConnectionInfo {
            client_addr: peer_addr,
            server_addr: local_addr,
            sock_peer_addr: peer_addr,
            sock_local_addr: local_addr,
            sock_raw_fd: raw_fd,
        }
    }

    #[inline]
    pub fn set_proxy_addr(&mut self, addr: ProxyAddr) {
        self.client_addr = addr.src_addr;
        self.server_addr = addr.dst_addr;
    }

    #[inline]
    pub fn client_addr(&self) -> SocketAddr {
        self.client_addr
    }

    pub fn client_ip(&self) -> IpAddr {
        self.client_addr.ip()
    }

    #[inline]
    pub fn server_addr(&self) -> SocketAddr {
        self.server_addr
    }

    pub fn server_ip(&self) -> IpAddr {
        self.server_addr.ip()
    }

    #[inline]
    pub fn sock_peer_addr(&self) -> SocketAddr {
        self.sock_peer_addr
    }

    pub fn sock_set_raw_opts(
        &self,
        opts: &TcpMiscSockOpts,
        default_set_nodelay: bool,
    ) -> io::Result<()> {
        g3_socket::tcp::set_raw_opts(self.sock_raw_fd, opts, default_set_nodelay)
    }
}
