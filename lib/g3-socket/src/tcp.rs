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
use std::os::unix::prelude::*;

use socket2::{Domain, SockAddr, Socket, TcpKeepalive, Type};
use tokio::net::{TcpListener, TcpSocket};

use g3_types::net::{TcpKeepAliveConfig, TcpListenConfig, TcpMiscSockOpts};

use super::sockopt::{set_bind_address_no_port, set_only_ipv6};
use super::util::AddressFamily;

pub fn new_std_listener(config: &TcpListenConfig) -> io::Result<std::net::TcpListener> {
    let addr = config.address();
    let socket = new_tcp_socket(AddressFamily::from(&addr))?;
    socket.set_nonblocking(true)?;
    socket.set_reuse_port(true)?;
    let addr: SockAddr = addr.into();
    if config.is_ipv6only() {
        socket.set_only_v6(true)?;
    }
    socket.bind(&addr)?;
    socket.listen(config.backlog() as i32)?;
    Ok(std::net::TcpListener::from(socket))
}

pub fn new_std_socket_to(
    peer_ip: IpAddr,
    bind_ip: Option<IpAddr>,
    keepalive: &TcpKeepAliveConfig,
    misc_opts: &TcpMiscSockOpts,
    default_set_nodelay: bool,
) -> io::Result<std::net::TcpStream> {
    let peer_family = AddressFamily::from(&peer_ip);
    let socket = new_tcp_socket(peer_family)?;
    socket.set_nonblocking(true)?;
    if let Some(ip) = bind_ip {
        if AddressFamily::from(&ip) != peer_family {
            return Err(io::Error::new(
                io::ErrorKind::InvalidInput,
                format!("peer_ip {peer_ip} and bind_ip {ip} should be of the same family",),
            ));
        }
        set_bind_address_no_port(socket.as_raw_fd(), true)?;
        let addr: SockAddr = SocketAddr::new(ip, 0).into();
        socket.bind(&addr)?;
    }
    if keepalive.is_enabled() {
        // set keepalive_idle
        let mut setting = TcpKeepalive::new().with_time(keepalive.idle_time());
        if let Some(interval) = keepalive.probe_interval() {
            setting = setting.with_interval(interval);
        }
        if let Some(count) = keepalive.probe_count() {
            setting = setting.with_retries(count);
        }
        socket.set_tcp_keepalive(&setting)?;
    }
    set_misc_opts(&socket, misc_opts, default_set_nodelay)?;
    Ok(std::net::TcpStream::from(socket))
}

pub fn set_raw_opts(
    fd: RawFd,
    misc_opts: &TcpMiscSockOpts,
    default_set_nodelay: bool,
) -> io::Result<()> {
    let socket = unsafe { Socket::from_raw_fd(fd) };
    set_misc_opts(&socket, misc_opts, default_set_nodelay)?;
    let _ = socket.into_raw_fd();
    Ok(())
}

fn set_misc_opts(
    socket: &Socket,
    misc_opts: &TcpMiscSockOpts,
    default_set_nodelay: bool,
) -> io::Result<()> {
    if let Some(no_delay) = misc_opts.no_delay {
        socket.set_nodelay(no_delay)?;
    } else if default_set_nodelay {
        socket.set_nodelay(true)?;
    }
    if let Some(mss) = misc_opts.max_segment_size {
        socket.set_mss(mss)?;
    }
    if let Some(ttl) = misc_opts.time_to_live {
        socket.set_ttl(ttl)?;
    }
    if let Some(tos) = misc_opts.type_of_service {
        socket.set_tos(tos as u32)?;
    }
    #[cfg(target_os = "linux")]
    if let Some(mark) = misc_opts.netfilter_mark {
        socket.set_mark(mark)?;
    }
    Ok(())
}

#[inline]
fn new_tcp_socket(family: AddressFamily) -> io::Result<Socket> {
    let socket = Socket::new(Domain::from(family), Type::STREAM, None)?;
    Ok(socket)
}

pub fn new_listen_to(config: &TcpListenConfig) -> io::Result<TcpListener> {
    let addr = config.address();
    let socket = match addr {
        SocketAddr::V4(_) => TcpSocket::new_v4()?,
        SocketAddr::V6(_) => TcpSocket::new_v6()?,
    };
    socket.set_reuseport(true)?;
    if config.is_ipv6only() {
        let raw_fd = socket.as_raw_fd();
        set_only_ipv6(raw_fd, true)?;
    }
    socket.bind(addr)?;
    socket.listen(config.backlog())
}

pub fn new_socket_to(
    peer_ip: IpAddr,
    bind_ip: Option<IpAddr>,
    keepalive: &TcpKeepAliveConfig,
    misc_opts: &TcpMiscSockOpts,
    default_set_nodelay: bool,
) -> io::Result<TcpSocket> {
    let socket = new_std_socket_to(peer_ip, bind_ip, keepalive, misc_opts, default_set_nodelay)?;
    Ok(TcpSocket::from_std_stream(socket))
}
