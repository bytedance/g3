/*
 * Copyright 2024 ByteDance and/or its affiliates.
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

use socket2::Socket;

pub(super) fn set_addr_reuse(socket: &Socket, addr: SocketAddr) -> io::Result<()> {
    if addr.port() != 0 {
        #[cfg(unix)]
        socket.set_reuse_address(true)?; // allow bind to local address if wildcard address is already bound
        #[cfg(any(target_os = "linux", target_os = "android", target_os = "dragonfly"))]
        socket.set_reuse_port(true)?; // load-balanced REUSE_PORT
        #[cfg(target_os = "freebsd")]
        socket.set_reuse_port_lb(true)?; // load-balanced REUSE_PORT like REUSE_PORT on DragonFly
        #[cfg(any(target_os = "netbsd", target_os = "openbsd", target_os = "macos"))]
        socket.set_reuse_port(true)?; // REUSE_PORT, the later will take over traffic?
        #[cfg(windows)]
        socket.set_reuse_address(true)?; // this is like REUSE_ADDR+REUSE_PORT on unix
    }
    Ok(())
}

pub(super) fn set_only_v6(socket: &Socket, addr: SocketAddr) -> io::Result<()> {
    match addr.ip() {
        IpAddr::V4(_) => Ok(()),
        IpAddr::V6(v6) => {
            if v6.is_unspecified() {
                socket.set_only_v6(true)
            } else {
                Ok(())
            }
        }
    }
}

pub(super) fn set_udp_recv_pktinfo(
    socket: &Socket,
    addr: SocketAddr,
    ipv6_only: bool,
) -> io::Result<()> {
    match addr.ip() {
        IpAddr::V4(v4) => {
            if !v4.is_unspecified() {
                return Ok(());
            }

            crate::sockopt::set_recv_ip_pktinfo(socket, true)
        }
        IpAddr::V6(v6) => {
            if !v6.is_unspecified() {
                return Ok(());
            }

            if !ipv6_only {
                if let Err(e) = crate::sockopt::set_recv_ip_pktinfo(socket, true) {
                    if e.kind() != io::ErrorKind::InvalidInput {
                        // the socket may already been ipv6 only so skip this error
                        return Err(e);
                    }
                }
            }
            crate::sockopt::set_recv_ipv6_pktinfo(socket, true)
        }
    }
}
