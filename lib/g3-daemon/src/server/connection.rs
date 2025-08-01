/*
 * SPDX-License-Identifier: Apache-2.0
 * Copyright 2023-2025 ByteDance and/or its affiliates.
 */

use std::io;
use std::net::{IpAddr, SocketAddr};

use g3_io_ext::haproxy::ProxyAddr;
use g3_socket::RawSocket;
use g3_socket::util::AddressFamily;
use g3_types::net::TcpMiscSockOpts;

#[derive(Clone, Debug)]
pub struct ClientConnectionInfo {
    worker_id: Option<usize>,
    client_addr: SocketAddr,
    server_addr: SocketAddr,
    sock_peer_addr: SocketAddr,
    #[allow(unused)]
    sock_local_addr: SocketAddr,
    tcp_raw_socket: Option<RawSocket>,
}

impl ClientConnectionInfo {
    pub fn new(peer_addr: SocketAddr, local_addr: SocketAddr) -> Self {
        ClientConnectionInfo {
            worker_id: None,
            client_addr: peer_addr,
            server_addr: local_addr,
            sock_peer_addr: peer_addr,
            sock_local_addr: local_addr,
            tcp_raw_socket: None,
        }
    }

    #[inline]
    pub fn set_tcp_raw_socket(&mut self, raw_fd: RawSocket) {
        self.tcp_raw_socket = Some(raw_fd);
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
        if let Some(raw_socket) = &self.tcp_raw_socket {
            raw_socket.set_tcp_misc_opts(
                AddressFamily::from(&self.client_addr),
                opts,
                default_set_nodelay,
            )
        } else {
            Ok(())
        }
    }

    #[cfg(any(target_os = "linux", target_os = "android", target_os = "illumos"))]
    pub fn tcp_sock_try_quick_ack(&self) {
        if let Some(raw_socket) = &self.tcp_raw_socket {
            let _ = raw_socket.trigger_tcp_quick_ack();
        }
    }

    #[cfg(not(any(target_os = "linux", target_os = "android", target_os = "illumos")))]
    pub fn tcp_sock_try_quick_ack(&self) {}

    #[cfg(any(target_os = "linux", target_os = "android"))]
    pub fn tcp_sock_incoming_cpu(&self) -> Option<usize> {
        if let Some(raw_socket) = &self.tcp_raw_socket {
            match raw_socket.tcp_incoming_cpu() {
                Ok(v) => Some(v),
                Err(e) => {
                    log::debug!("failed to get incoming cpu of socket: {e}");
                    None
                }
            }
        } else {
            None
        }
    }
}
