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
    worker_id: Option<usize>,
    client_addr: SocketAddr,
    server_addr: SocketAddr,
    sock_peer_addr: SocketAddr,
    #[allow(unused)]
    sock_local_addr: SocketAddr,
    tcp_sock_raw_fd: Option<RawFd>,
}

impl ClientConnectionInfo {
    pub fn new(peer_addr: SocketAddr, local_addr: SocketAddr) -> Self {
        ClientConnectionInfo {
            worker_id: None,
            client_addr: peer_addr,
            server_addr: local_addr,
            sock_peer_addr: peer_addr,
            sock_local_addr: local_addr,
            tcp_sock_raw_fd: None,
        }
    }

    #[inline]
    pub fn set_tcp_raw_fd(&mut self, raw_fd: RawFd) {
        self.tcp_sock_raw_fd = Some(raw_fd);
    }

    #[inline]
    pub fn set_proxy_addr(&mut self, addr: ProxyAddr) {
        self.client_addr = addr.src_addr;
        self.server_addr = addr.dst_addr;
    }

    #[inline]
    pub fn set_worker_id(&mut self, worker_id: Option<usize>) {
        self.worker_id = worker_id;
    }

    #[inline]
    pub fn worker_id(&self) -> Option<usize> {
        self.worker_id
    }

    #[inline]
    pub fn client_addr(&self) -> SocketAddr {
        self.client_addr
    }

    #[inline]
    pub fn client_ip(&self) -> IpAddr {
        self.client_addr.ip()
    }

    #[inline]
    pub fn server_addr(&self) -> SocketAddr {
        self.server_addr
    }

    #[inline]
    pub fn server_ip(&self) -> IpAddr {
        self.server_addr.ip()
    }

    #[inline]
    pub fn sock_peer_addr(&self) -> SocketAddr {
        self.sock_peer_addr
    }

    #[inline]
    pub fn sock_peer_ip(&self) -> IpAddr {
        self.sock_peer_addr.ip()
    }

    #[inline]
    pub fn sock_local_addr(&self) -> SocketAddr {
        self.sock_local_addr
    }

    pub fn tcp_sock_set_raw_opts(
        &self,
        opts: &TcpMiscSockOpts,
        default_set_nodelay: bool,
    ) -> io::Result<()> {
        if let Some(raw_fd) = self.tcp_sock_raw_fd {
            g3_socket::tcp::set_raw_opts(raw_fd, opts, default_set_nodelay)
        } else {
            Ok(())
        }
    }
}
