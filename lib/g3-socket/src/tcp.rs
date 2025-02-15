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

use g3_compat::CpuAffinity;
use g3_types::net::{TcpKeepAliveConfig, TcpListenConfig, TcpMiscSockOpts};

use super::util::AddressFamily;
use super::{BindAddr, RawSocket};

pub fn new_std_listener(config: &TcpListenConfig) -> io::Result<std::net::TcpListener> {
    let addr = config.address();
    let socket = new_tcp_socket(AddressFamily::from(&addr))?;
    super::listen::set_addr_reuse(&socket, addr)?;
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
    #[cfg(windows)]
    if keepalive.is_enabled() {
        // set keepalive_idle
        let mut setting = TcpKeepalive::new().with_time(keepalive.idle_time());
        if let Some(interval) = keepalive.probe_interval() {
            setting = setting.with_interval(interval);
        }
        socket.set_tcp_keepalive(&setting)?;
    }
    #[cfg(all(unix, not(target_os = "openbsd")))]
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
    #[cfg(target_os = "openbsd")]
    if keepalive.is_enabled() {
        // set keepalive_idle
        let setting = TcpKeepalive::new().with_time(keepalive.idle_time());
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
}
