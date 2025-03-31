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
use std::net::{IpAddr, Ipv6Addr, SocketAddr, UdpSocket};

use socket2::{Domain, SockAddr, Socket, Type};

use g3_types::net::{PortRange, SocketBufferConfig, UdpListenConfig, UdpMiscSockOpts};

use super::util::AddressFamily;
use super::{BindAddr, RawSocket};

pub fn new_std_socket_to(
    peer_addr: SocketAddr,
    bind: &BindAddr,
    buf_conf: SocketBufferConfig,
    misc_opts: UdpMiscSockOpts,
) -> io::Result<UdpSocket> {
    let peer_family = AddressFamily::from(&peer_addr);
    let socket = new_udp_socket(peer_family, buf_conf)?;
    bind.bind_for_connect(&socket, peer_family)?;
    RawSocket::from(&socket).set_udp_misc_opts(misc_opts)?;
    Ok(UdpSocket::from(socket))
}

pub fn new_std_bind_lazy_connect(
    bind_ip: Option<IpAddr>,
    buf_conf: SocketBufferConfig,
    misc_opts: UdpMiscSockOpts,
) -> io::Result<(UdpSocket, SocketAddr)> {
    let bind_addr = match bind_ip {
        Some(ip) => SocketAddr::new(ip, 0),
        None => SocketAddr::new(IpAddr::V6(Ipv6Addr::UNSPECIFIED), 0),
    };
    let socket = new_udp_socket(AddressFamily::from(&bind_addr), buf_conf)?;
    RawSocket::from(&socket).set_udp_misc_opts(misc_opts)?;
    let bind_addr = SockAddr::from(bind_addr);
    socket.bind(&bind_addr)?;
    let socket = UdpSocket::from(socket);
    let listen_addr = socket.local_addr()?;

    Ok((socket, listen_addr))
}

pub fn new_std_in_range_bind_lazy_connect(
    bind_ip: IpAddr,
    port: PortRange,
    buf_conf: SocketBufferConfig,
    misc_opts: UdpMiscSockOpts,
) -> io::Result<(UdpSocket, SocketAddr)> {
    let port_start = port.start();
    let port_end = port.end();

    debug_assert!(port_start < port_end);

    let socket = new_udp_socket(AddressFamily::from(&bind_ip), buf_conf)?;
    RawSocket::from(&socket).set_udp_misc_opts(misc_opts)?;

    // like what's has been done in dante/sockd/sockd_request.c
    let tries = port.count().min(10);
    for _i in 0..tries {
        let port = fastrand::u16(port_start..=port_end);
        let bind_addr: SockAddr = SocketAddr::new(bind_ip, port).into();
        if socket.bind(&bind_addr).is_ok() {
            let socket = UdpSocket::from(socket);
            let listen_addr = socket.local_addr()?;
            return Ok((socket, listen_addr));
        }
    }

    for port in port_start..=port_end {
        let bind_addr: SockAddr = SocketAddr::new(bind_ip, port).into();
        if socket.bind(&bind_addr).is_ok() {
            let socket = UdpSocket::from(socket);
            let listen_addr = socket.local_addr()?;
            return Ok((socket, listen_addr));
        }
    }

    Err(io::Error::new(
        io::ErrorKind::AddrNotAvailable,
        "no port can be selected within specified range",
    ))
}

pub fn new_std_bind_relay(
    bind: &BindAddr,
    family: AddressFamily,
    buf_conf: SocketBufferConfig,
    misc_opts: UdpMiscSockOpts,
) -> io::Result<UdpSocket> {
    let socket = new_udp_socket(family, buf_conf)?;
    bind.bind_for_relay(&socket, family)?;
    RawSocket::from(&socket).set_udp_misc_opts(misc_opts)?;
    Ok(UdpSocket::from(socket))
}

pub fn new_std_bind_listen(config: &UdpListenConfig) -> io::Result<UdpSocket> {
    let addr = config.address();
    let socket = new_udp_socket(AddressFamily::from(&addr), config.socket_buffer())?;
    super::listen::set_addr_reuse(&socket, addr)?;
    if let Some(enable) = config.is_ipv6only() {
        super::listen::set_only_v6(&socket, addr, enable)?;
    }
    let bind_addr = SockAddr::from(addr);
    socket.bind(&bind_addr)?;
    #[cfg(unix)]
    super::listen::set_udp_recv_pktinfo(&socket, addr)?;
    #[cfg(windows)]
    super::listen::set_udp_recv_pktinfo(&socket, addr, config.is_ipv6only())?;
    RawSocket::from(&socket).set_udp_misc_opts(config.socket_misc_opts())?;
    Ok(UdpSocket::from(socket))
}

pub fn new_std_rebind_listen(config: &UdpListenConfig, addr: SocketAddr) -> io::Result<UdpSocket> {
    let socket = new_udp_socket(AddressFamily::from(&addr), config.socket_buffer())?;
    super::listen::set_addr_reuse(&socket, addr)?;
    if let Some(enable) = config.is_ipv6only() {
        super::listen::set_only_v6(&socket, addr, enable)?;
    }
    let bind_addr = SockAddr::from(addr);
    socket.bind(&bind_addr)?;
    #[cfg(unix)]
    super::listen::set_udp_recv_pktinfo(&socket, addr)?;
    #[cfg(windows)]
    super::listen::set_udp_recv_pktinfo(&socket, addr, config.is_ipv6only())?;
    RawSocket::from(&socket).set_udp_misc_opts(config.socket_misc_opts())?;
    Ok(UdpSocket::from(socket))
}

fn new_udp_socket(family: AddressFamily, buf_conf: SocketBufferConfig) -> io::Result<Socket> {
    let socket = new_nonblocking_udp_socket(family)?;
    RawSocket::from(&socket).set_buf_opts(buf_conf)?;
    Ok(socket)
}

#[cfg(any(windows, target_os = "macos"))]
fn new_nonblocking_udp_socket(family: AddressFamily) -> io::Result<Socket> {
    let socket = Socket::new(Domain::from(family), Type::DGRAM, None)?;
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
    target_os = "illumos",
    target_os = "solaris",
))]
fn new_nonblocking_udp_socket(family: AddressFamily) -> io::Result<Socket> {
    Socket::new(Domain::from(family), Type::DGRAM.nonblocking(), None)
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::net::Ipv4Addr;
    use std::str::FromStr;

    #[test]
    fn bind_later() {
        let peer_addr = SocketAddr::from_str("127.0.0.1:514").unwrap();
        let socket = new_std_socket_to(
            peer_addr,
            &BindAddr::Ip(IpAddr::V4(Ipv4Addr::UNSPECIFIED)),
            SocketBufferConfig::default(),
            Default::default(),
        )
        .unwrap();
        let local_addr1 = socket.local_addr().unwrap();
        assert_eq!(local_addr1.ip(), IpAddr::V4(Ipv4Addr::UNSPECIFIED));
        #[cfg(any(target_os = "linux", target_os = "android"))]
        assert_eq!(local_addr1.port(), 0);
        #[cfg(not(any(target_os = "linux", target_os = "android")))]
        assert_ne!(local_addr1.port(), 0);
        socket.connect(peer_addr).unwrap();
        let local_addr2 = socket.local_addr().unwrap();
        assert_ne!(local_addr2.port(), 0);
        assert_ne!(local_addr1, local_addr2);
    }

    #[test]
    fn bind_to_ip() {
        let (_socket, local_addr) = new_std_bind_lazy_connect(
            Some(IpAddr::V4(Ipv4Addr::UNSPECIFIED)),
            SocketBufferConfig::default(),
            Default::default(),
        )
        .unwrap();
        assert_ne!(local_addr.port(), 0);
    }

    #[test]
    fn bind_in_range() {
        let port_start = 61000;
        let port_end = 65000;
        let range = PortRange::new(port_start, port_end);
        let ip = IpAddr::V4(Ipv4Addr::LOCALHOST);
        let loop_len = 100usize;
        let mut v = Vec::<UdpSocket>::with_capacity(loop_len);
        for _i in 0..loop_len {
            let (socket, local_addr) = new_std_in_range_bind_lazy_connect(
                ip,
                range,
                SocketBufferConfig::default(),
                Default::default(),
            )
            .unwrap();
            let port_real = local_addr.port();
            assert!(port_real >= port_start);
            assert!(port_real <= port_end);
            v.push(socket);
        }
    }

    #[test]
    fn listen() {
        let mut config = UdpListenConfig::default();

        let socket = new_std_bind_listen(&config).unwrap();
        let local_addr = socket.local_addr().unwrap();
        assert_ne!(local_addr.port(), 0);
        assert!(local_addr.ip().is_unspecified());
        drop(socket);

        config.set_ipv6_only(false);
        let socket = new_std_bind_listen(&config).unwrap();
        let local_addr = socket.local_addr().unwrap();
        assert_ne!(local_addr.port(), 0);
        assert!(local_addr.ip().is_unspecified());
        drop(socket);

        config.set_ipv6_only(true);
        let socket = new_std_bind_listen(&config).unwrap();
        let local_addr = socket.local_addr().unwrap();
        assert_ne!(local_addr.port(), 0);
        assert!(local_addr.ip().is_unspecified());
        drop(socket);

        config.set_socket_address(SocketAddr::from_str("0.0.0.0:0").unwrap());
        config.set_ipv6_only(false);
        let socket = new_std_bind_listen(&config).unwrap();
        let local_addr = socket.local_addr().unwrap();
        assert_ne!(local_addr.port(), 0);
        assert!(local_addr.ip().is_unspecified());
        drop(socket);
    }
}
