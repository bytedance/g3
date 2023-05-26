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
use std::net::{IpAddr, Ipv4Addr, Ipv6Addr, SocketAddr, UdpSocket};
use std::os::unix::prelude::*;

use rand::distributions::Uniform;
use rand::Rng;
use socket2::{Domain, SockAddr, Socket, Type};

use g3_types::net::{PortRange, SocketBufferConfig, UdpMiscSockOpts};

use super::sockopt::set_bind_address_no_port;
use super::util::AddressFamily;

pub fn new_std_socket_to(
    peer_addr: SocketAddr,
    bind_ip: Option<IpAddr>,
    buf_conf: SocketBufferConfig,
    misc_opts: &UdpMiscSockOpts,
) -> io::Result<UdpSocket> {
    let peer_family = AddressFamily::from(&peer_addr);
    let socket = new_udp_socket(peer_family, buf_conf)?;
    socket.set_nonblocking(true)?;
    if let Some(ip) = bind_ip {
        if AddressFamily::from(&ip) != peer_family {
            return Err(io::Error::new(
                io::ErrorKind::InvalidInput,
                format!("peer_addr {peer_addr} and bind_ip {ip} should be of the same family",),
            ));
        }
        set_bind_address_no_port(socket.as_raw_fd(), true)?;
        let addr: SockAddr = SocketAddr::new(ip, 0).into();
        socket.bind(&addr)?;
    }
    set_misc_opts(&socket, misc_opts)?;
    Ok(UdpSocket::from(socket))
}

pub fn new_std_bind_connect(
    bind_ip: Option<IpAddr>,
    buf_conf: SocketBufferConfig,
    misc_opts: &UdpMiscSockOpts,
) -> io::Result<(UdpSocket, SocketAddr)> {
    let bind_addr = match bind_ip {
        Some(ip) => SocketAddr::new(ip, 0),
        None => SocketAddr::new(IpAddr::V6(Ipv6Addr::UNSPECIFIED), 0),
    };
    let socket = new_udp_socket(AddressFamily::from(&bind_addr), buf_conf)?;
    socket.set_nonblocking(true)?;
    set_misc_opts(&socket, misc_opts)?;
    let bind_addr = SockAddr::from(bind_addr);
    socket.bind(&bind_addr)?;
    let socket = UdpSocket::from(socket);
    let listen_addr = socket.local_addr()?;

    Ok((socket, listen_addr))
}

pub fn new_std_in_range_bind_connect(
    bind_ip: IpAddr,
    port: PortRange,
    buf_conf: SocketBufferConfig,
    misc_opts: &UdpMiscSockOpts,
) -> io::Result<(UdpSocket, SocketAddr)> {
    let port_start = port.start();
    let port_end = port.end();

    debug_assert!(port_start < port_end);

    let socket = new_udp_socket(AddressFamily::from(&bind_ip), buf_conf)?;
    socket.set_nonblocking(true)?;
    set_misc_opts(&socket, misc_opts)?;

    // like what's has been done in dante/sockd/sockd_request.c
    let side = Uniform::new_inclusive(port_start, port_end);
    let mut rng = rand::thread_rng();
    let tries = port.count().min(10);
    for _i in 0..tries {
        let port = rng.sample(side);
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
    bind_ip: Option<IpAddr>,
    family: AddressFamily,
    buf_conf: SocketBufferConfig,
    misc_opts: &UdpMiscSockOpts,
) -> io::Result<UdpSocket> {
    let bind_addr = match bind_ip {
        Some(ip) => SocketAddr::new(ip, 0),
        None => match family {
            AddressFamily::Ipv4 => SocketAddr::new(IpAddr::V4(Ipv4Addr::UNSPECIFIED), 0),
            AddressFamily::Ipv6 => SocketAddr::new(IpAddr::V6(Ipv6Addr::UNSPECIFIED), 0),
        },
    };
    let socket = new_udp_socket(AddressFamily::from(&bind_addr), buf_conf)?;
    socket.set_nonblocking(true)?;
    let bind_addr = SockAddr::from(bind_addr);
    socket.bind(&bind_addr)?;
    set_misc_opts(&socket, misc_opts)?;
    Ok(UdpSocket::from(socket))
}

pub fn set_raw_opts(fd: RawFd, misc_opts: &UdpMiscSockOpts) -> io::Result<()> {
    let socket = unsafe { Socket::from_raw_fd(fd) };
    set_misc_opts(&socket, misc_opts)?;
    let _ = socket.into_raw_fd();
    Ok(())
}

fn set_misc_opts(socket: &Socket, misc_opts: &UdpMiscSockOpts) -> io::Result<()> {
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

fn new_udp_socket(family: AddressFamily, buf_conf: SocketBufferConfig) -> io::Result<Socket> {
    let socket = Socket::new(Domain::from(family), Type::DGRAM, None)?;
    if let Some(size) = buf_conf.recv_size() {
        socket.set_recv_buffer_size(size)?;
    }
    if let Some(size) = buf_conf.send_size() {
        socket.set_send_buffer_size(size)?;
    }
    Ok(socket)
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::net::{IpAddr, Ipv4Addr};
    use std::str::FromStr;

    #[test]
    fn bind_later() {
        let peer_addr = SocketAddr::from_str("127.0.0.1:514").unwrap();
        let socket = new_std_socket_to(
            peer_addr,
            Some(IpAddr::V4(Ipv4Addr::UNSPECIFIED)),
            SocketBufferConfig::default(),
            &Default::default(),
        )
        .unwrap();
        let local_addr = socket.local_addr().unwrap();
        assert_eq!(local_addr.port(), 0);
        socket.connect(peer_addr).unwrap();
        let local_addr = socket.local_addr().unwrap();
        assert_ne!(local_addr.port(), 0);
    }

    #[test]
    fn bind_to_ip() {
        let (_socket, local_addr) = new_std_bind_connect(
            Some(IpAddr::V4(Ipv4Addr::UNSPECIFIED)),
            SocketBufferConfig::default(),
            &Default::default(),
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
            let (socket, local_addr) = new_std_in_range_bind_connect(
                ip,
                range,
                SocketBufferConfig::default(),
                &Default::default(),
            )
            .unwrap();
            let port_real = local_addr.port();
            assert!(port_real >= port_start);
            assert!(port_real <= port_end);
            v.push(socket);
        }
    }
}
