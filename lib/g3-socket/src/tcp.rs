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
use std::net::IpAddr;

use socket2::{Domain, SockAddr, Socket, TcpKeepalive, Type};
use tokio::net::{TcpListener, TcpSocket};

use g3_types::net::{TcpKeepAliveConfig, TcpListenConfig, TcpMiscSockOpts};

use super::util::AddressFamily;
use super::{BindAddr, RawSocket};

pub fn new_std_listener(config: &TcpListenConfig) -> io::Result<std::net::TcpListener> {
    let addr = config.address();
    let socket = new_tcp_socket(AddressFamily::from(&addr))?;
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
    if config.is_ipv6only() {
        socket.set_only_v6(true)?;
    }
    #[cfg(target_os = "linux")]
    if config.transparent() {
        socket.set_ip_transparent(true)?;
    }
    #[cfg(any(target_os = "android", target_os = "fuchsia", target_os = "linux"))]
    if let Some(mark) = config.mark() {
        socket.set_mark(mark)?;
    }
    let bind_addr: SockAddr = addr.into();
    socket.bind(&bind_addr)?;
    socket.listen(config.backlog() as i32)?;
    Ok(std::net::TcpListener::from(socket))
}

pub fn new_std_socket_to(
    peer_ip: IpAddr,
    bind: &BindAddr,
    keepalive: &TcpKeepAliveConfig,
    misc_opts: &TcpMiscSockOpts,
    default_set_nodelay: bool,
) -> io::Result<std::net::TcpStream> {
    let peer_family = AddressFamily::from(&peer_ip);
    let socket = new_tcp_socket(peer_family)?;
    bind.bind_for_connect(&socket, peer_family)?;
    if keepalive.is_enabled() {
        // set keepalive_idle
        let mut setting = TcpKeepalive::new().with_time(keepalive.idle_time());
        if let Some(interval) = keepalive.probe_interval() {
            setting = setting.with_interval(interval);
        }
        #[cfg(unix)]
        if let Some(count) = keepalive.probe_count() {
            setting = setting.with_retries(count);
        }
        socket.set_tcp_keepalive(&setting)?;
    }
    RawSocket::from(&socket).set_tcp_misc_opts(misc_opts, default_set_nodelay)?;
    Ok(std::net::TcpStream::from(socket))
}

#[cfg(any(windows, target_os = "macos"))]
fn new_tcp_socket(family: AddressFamily) -> io::Result<Socket> {
    let socket = Socket::new(Domain::from(family), Type::STREAM, None)?;
    socket.set_nonblocking(true)?;
    Ok(socket)
}

#[cfg(any(
    target_os = "linux",
    target_os = "android",
    target_os = "freebsd",
    target_os = "dragonfly",
    target_os = "netbsd",
    target_os = "openbsd",
))]
fn new_tcp_socket(family: AddressFamily) -> io::Result<Socket> {
    Socket::new(Domain::from(family), Type::STREAM.nonblocking(), None)
}

pub fn new_listen_to(config: &TcpListenConfig) -> io::Result<TcpListener> {
    let socket = new_std_listener(config)?;
    TcpListener::from_std(socket)
}

pub fn new_socket_to(
    peer_ip: IpAddr,
    bind: &BindAddr,
    keepalive: &TcpKeepAliveConfig,
    misc_opts: &TcpMiscSockOpts,
    default_set_nodelay: bool,
) -> io::Result<TcpSocket> {
    let socket = new_std_socket_to(peer_ip, bind, keepalive, misc_opts, default_set_nodelay)?;
    Ok(TcpSocket::from_std_stream(socket))
}
