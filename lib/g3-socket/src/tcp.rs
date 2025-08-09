/*
 * SPDX-License-Identifier: Apache-2.0
 * Copyright 2023-2025 ByteDance and/or its affiliates.
 */

use std::io;
use std::net::IpAddr;

use socket2::{Domain, SockAddr, Socket, TcpKeepalive, Type};
use tokio::net::{TcpListener, TcpSocket};

use g3_compat::CpuAffinity;
use g3_types::net::{TcpKeepAliveConfig, TcpListenConfig, TcpMiscSockOpts};

use super::util::AddressFamily;
use super::{BindAddr, RawSocket};

pub fn new_std_listener(config: &TcpListenConfig) -> io::Result<std::net::TcpListener> {
    let addr = config.address();
    let family = AddressFamily::from(&addr);
    let socket = new_tcp_socket(family)?;
    super::listen::set_addr_reuse(&socket, addr)?;
    // OpenBSD is always ipv6-only
    #[cfg(not(target_os = "openbsd"))]
    if let Some(enable) = config.is_ipv6only() {
        super::listen::set_only_v6(&socket, addr, enable)?;
    }
    #[cfg(target_os = "linux")]
    if config.transparent() {
        match family {
            AddressFamily::Ipv4 => {
                socket.set_ip_transparent_v4(true)?;
            }
            AddressFamily::Ipv6 => {
                crate::sockopt::set_ip_transparent_v6(&socket, true)?;
            }
        }
    }
    #[cfg(any(target_os = "android", target_os = "fuchsia", target_os = "linux"))]
    if let Some(mark) = config.mark() {
        socket.set_mark(mark)?;
    }
    let bind_addr: SockAddr = addr.into();
    socket.bind(&bind_addr)?;
    #[cfg(any(target_os = "linux", target_os = "android"))]
    if let Some(iface) = config.interface() {
        socket.bind_device(Some(iface.c_bytes()))?;
    }

    if let Some(keepalive_config) = config.keepalive()
        && let Some(setting) = enable_tcp_keepalive(keepalive_config)
    {
        socket.set_tcp_keepalive(&setting)?;
    }

    #[cfg(any(target_os = "macos", target_os = "illumos", target_os = "solaris"))]
    if let Some(iface) = config.interface() {
        match family {
            AddressFamily::Ipv4 => socket.bind_device_by_index_v4(Some(iface.id()))?,
            AddressFamily::Ipv6 => socket.bind_device_by_index_v6(Some(iface.id()))?,
        }
    }
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
    bind.bind_tcp_for_connect(&socket, peer_family)?;

    if let Some(setting) = enable_tcp_keepalive(keepalive) {
        socket.set_tcp_keepalive(&setting)?;
    }

    RawSocket::from(&socket).set_tcp_misc_opts(peer_family, misc_opts, default_set_nodelay)?;
    Ok(std::net::TcpStream::from(socket))
}

#[cfg(not(target_os = "openbsd"))]
fn enable_tcp_keepalive(config: &TcpKeepAliveConfig) -> Option<TcpKeepalive> {
    if config.is_enabled() {
        let mut setting = TcpKeepalive::new().with_time(config.idle_time());
        if let Some(interval) = config.probe_interval() {
            setting = setting.with_interval(interval);
        }
        if let Some(count) = config.probe_count() {
            setting = setting.with_retries(count);
        }
        Some(setting)
    } else {
        None
    }
}

#[cfg(target_os = "openbsd")]
fn enable_tcp_keepalive(config: &TcpKeepAliveConfig) -> Option<TcpKeepalive> {
    if config.is_enabled() {
        let keepalive = TcpKeepalive::new().with_time(config.idle_time());
        Some(keepalive)
    } else {
        None
    }
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
    target_os = "illumos",
    target_os = "solaris",
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

#[cfg(target_os = "linux")]
pub fn try_listen_on_local_cpu(
    listener: &std::net::TcpListener,
    cpu_affinity: &CpuAffinity,
) -> io::Result<()> {
    let cpu_id_list = cpu_affinity.cpu_id_list();
    if cpu_id_list.len() == 1 {
        let cpu_id = cpu_id_list[0];
        super::sockopt::set_incoming_cpu(listener, cpu_id)
    } else {
        Ok(())
    }
}

#[cfg(target_os = "freebsd")]
pub fn try_listen_on_local_cpu(
    listener: &std::net::TcpListener,
    cpu_affinity: &CpuAffinity,
) -> io::Result<()> {
    let cpu_id_list = cpu_affinity.cpu_id_list();
    if !cpu_id_list.is_empty() {
        // NOTE: we don't check if all the CPU ids on the same NUMA here
        super::sockopt::set_tcp_reuseport_lb_numa_current_domain(listener)
    } else {
        Ok(())
    }
}

#[cfg(not(any(target_os = "linux", target_os = "freebsd")))]
pub fn try_listen_on_local_cpu(
    _listener: &std::net::TcpListener,
    _cpu_affinity: &CpuAffinity,
) -> io::Result<()> {
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::net::{Ipv4Addr, SocketAddr};

    #[tokio::test]
    async fn listen_connect() {
        let listen_config =
            TcpListenConfig::new(SocketAddr::new(IpAddr::V4(Ipv4Addr::LOCALHOST), 0));
        let listen_socket = new_listen_to(&listen_config).unwrap();
        let listen_addr = listen_socket.local_addr().unwrap();

        let accept_task = tokio::spawn(async move {
            let (_stream, accepted_addr) = listen_socket.accept().await.unwrap();
            accepted_addr
        });

        let connect_sock = new_socket_to(
            listen_addr.ip(),
            &BindAddr::None,
            &TcpKeepAliveConfig::default(),
            &TcpMiscSockOpts::default(),
            true,
        )
        .unwrap();
        let connected_stream = connect_sock.connect(listen_addr).await.unwrap();
        let connect_addr = connected_stream.local_addr().unwrap();
        let accepted_addr = accept_task.await.unwrap();
        assert_eq!(connect_addr, accepted_addr);
    }

    #[tokio::test]
    async fn bind_connect() {
        let listen_config =
            TcpListenConfig::new(SocketAddr::new(IpAddr::V4(Ipv4Addr::LOCALHOST), 0));
        let listen_socket = new_listen_to(&listen_config).unwrap();
        let listen_addr = listen_socket.local_addr().unwrap();

        let accept_task = tokio::spawn(async move {
            let (_stream, accepted_addr) = listen_socket.accept().await.unwrap();
            accepted_addr
        });

        let bind = BindAddr::Ip(IpAddr::V4(Ipv4Addr::LOCALHOST));
        let connect_sock = new_socket_to(
            listen_addr.ip(),
            &bind,
            &TcpKeepAliveConfig::default(),
            &TcpMiscSockOpts::default(),
            true,
        )
        .unwrap();
        let connected_stream = connect_sock.connect(listen_addr).await.unwrap();
        let connect_addr = connected_stream.local_addr().unwrap();
        let accepted_addr = accept_task.await.unwrap();
        assert_eq!(connect_addr, accepted_addr);
    }
}
