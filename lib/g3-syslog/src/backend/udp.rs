/*
 * SPDX-License-Identifier: Apache-2.0
 * Copyright 2023-2025 ByteDance and/or its affiliates.
 */

use std::io;
use std::net::{IpAddr, Ipv6Addr, SocketAddr, UdpSocket};

pub(crate) fn udp(bind_ip: Option<IpAddr>, server: SocketAddr) -> io::Result<UdpSocket> {
    let bind_addr = match bind_ip {
        Some(ip) => SocketAddr::new(ip, 0),
        None => SocketAddr::new(IpAddr::V6(Ipv6Addr::UNSPECIFIED), 0),
    };
    let sock = UdpSocket::bind(bind_addr)?;
    sock.connect(server)?;
    Ok(sock)
}
