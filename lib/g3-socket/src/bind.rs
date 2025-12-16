/*
 * SPDX-License-Identifier: Apache-2.0
 * Copyright 2024-2025 ByteDance and/or its affiliates.
 */

use std::io;
use std::net::{IpAddr, Ipv4Addr, Ipv6Addr, SocketAddr};

use socket2::{SockAddr, Socket};

#[cfg(any(
    target_os = "linux",
    target_os = "android",
    target_os = "macos",
    target_os = "illumos",
    target_os = "solaris"
))]
use g3_types::net::Interface;

#[cfg(any(target_os = "linux", target_os = "android"))]
use super::sockopt::set_bind_address_no_port;
#[cfg(windows)]
use super::sockopt::set_reuse_unicastport;
use crate::util::AddressFamily;

#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub enum BindAddr {
    #[default]
    None,
    Ip(IpAddr),
    #[cfg(any(
        target_os = "linux",
        target_os = "android",
        target_os = "macos",
        target_os = "illumos",
        target_os = "solaris"
    ))]
    Interface(Interface),
    #[cfg(target_os = "linux")]
    Foreign(SocketAddr),
}

impl BindAddr {
    pub fn is_none(&self) -> bool {
        matches!(self, BindAddr::None)
    }

    pub fn ip(&self) -> Option<IpAddr> {
        match self {
            BindAddr::None => None,
            BindAddr::Ip(ip) => Some(*ip),
            #[cfg(any(
                target_os = "linux",
                target_os = "android",
                target_os = "macos",
                target_os = "illumos",
                target_os = "solaris"
            ))]
            BindAddr::Interface(_) => None,
            #[cfg(target_os = "linux")]
            BindAddr::Foreign(addr) => Some(addr.ip()),
        }
    }

    pub(crate) fn bind_tcp_for_connect(
        &self,
        socket: &Socket,
        peer_family: AddressFamily,
    ) -> io::Result<()> {
        match self {
            BindAddr::None => Ok(()),
            BindAddr::Ip(ip) => {
                if AddressFamily::from(ip) != peer_family {
                    return Err(io::Error::new(
                        io::ErrorKind::InvalidInput,
                        "bind_ip should be of the same family with peer ip",
                    ));
                }
                #[cfg(any(target_os = "linux", target_os = "android"))]
                set_bind_address_no_port(socket, true)?;
                #[cfg(windows)]
                set_reuse_unicastport(socket, true)?;
                let addr: SockAddr = SocketAddr::new(*ip, 0).into();
                socket.bind(&addr)
            }
            #[cfg(any(target_os = "linux", target_os = "android"))]
            BindAddr::Interface(iface) => {
                set_bind_address_no_port(socket, true)?;
                socket.bind_device(Some(iface.c_bytes()))
            }
            #[cfg(any(target_os = "macos", target_os = "illumos", target_os = "solaris"))]
            BindAddr::Interface(iface) => match peer_family {
                AddressFamily::Ipv4 => socket.bind_device_by_index_v4(Some(iface.id())),
                AddressFamily::Ipv6 => socket.bind_device_by_index_v6(Some(iface.id())),
            },
            #[cfg(target_os = "linux")]
            BindAddr::Foreign(addr) => {
                if AddressFamily::from(addr) != peer_family {
                    return Err(io::Error::new(
                        io::ErrorKind::InvalidInput,
                        "foreign bind addr should be of the same family with peer ip",
                    ));
                }
                if addr.port() == 0 {
                    set_bind_address_no_port(socket, true)?;
                } else {
                    socket.set_reuse_address(true)?;
                }
                match addr {
                    SocketAddr::V4(_) => {
                        socket.set_ip_transparent_v4(true)?;
                    }
                    SocketAddr::V6(_) => {
                        crate::sockopt::set_ip_transparent_v6(socket, true)?;
                    }
                }
                let addr: SockAddr = (*addr).into();
                socket.bind(&addr)
            }
        }
    }

    pub(crate) fn bind_udp_for_connect(
        &self,
        socket: &Socket,
        peer_family: AddressFamily,
    ) -> io::Result<()> {
        match self {
            BindAddr::None => Ok(()),
            BindAddr::Ip(ip) => {
                if AddressFamily::from(ip) != peer_family {
                    return Err(io::Error::new(
                        io::ErrorKind::InvalidInput,
                        "bind_ip should be of the same family with peer ip",
                    ));
                }
                #[cfg(any(target_os = "linux", target_os = "android"))]
                set_bind_address_no_port(socket, true)?;
                // SO_REUSE_UNICASTPORT is not available for UDP socket on Windows
                let addr: SockAddr = SocketAddr::new(*ip, 0).into();
                socket.bind(&addr)
            }
            #[cfg(any(target_os = "linux", target_os = "android"))]
            BindAddr::Interface(iface) => {
                set_bind_address_no_port(socket, true)?;
                socket.bind_device(Some(iface.c_bytes()))
            }
            #[cfg(any(target_os = "macos", target_os = "illumos", target_os = "solaris"))]
            BindAddr::Interface(iface) => match peer_family {
                AddressFamily::Ipv4 => socket.bind_device_by_index_v4(Some(iface.id())),
                AddressFamily::Ipv6 => socket.bind_device_by_index_v6(Some(iface.id())),
            },
            #[cfg(target_os = "linux")]
            BindAddr::Foreign(addr) => {
                if AddressFamily::from(addr) != peer_family {
                    return Err(io::Error::new(
                        io::ErrorKind::InvalidInput,
                        "foreign bind addr should be of the same family with peer ip",
                    ));
                }
                if addr.port() == 0 {
                    set_bind_address_no_port(socket, true)?;
                }
                match addr {
                    SocketAddr::V4(_) => {
                        socket.set_ip_transparent_v4(true)?;
                    }
                    SocketAddr::V6(_) => {
                        crate::sockopt::set_ip_transparent_v6(socket, true)?;
                    }
                }
                let addr: SockAddr = (*addr).into();
                socket.bind(&addr)
            }
        }
    }

    pub(crate) fn bind_for_relay(&self, socket: &Socket, family: AddressFamily) -> io::Result<()> {
        let bind_ip = match self {
            BindAddr::None => match family {
                AddressFamily::Ipv4 => IpAddr::V4(Ipv4Addr::UNSPECIFIED),
                AddressFamily::Ipv6 => IpAddr::V6(Ipv6Addr::UNSPECIFIED),
            },
            BindAddr::Ip(ip) => *ip,
            #[cfg(any(target_os = "linux", target_os = "android"))]
            BindAddr::Interface(iface) => {
                socket.bind_device(Some(iface.c_bytes()))?;
                match family {
                    AddressFamily::Ipv4 => IpAddr::V4(Ipv4Addr::UNSPECIFIED),
                    AddressFamily::Ipv6 => IpAddr::V6(Ipv6Addr::UNSPECIFIED),
                }
            }
            #[cfg(any(target_os = "macos", target_os = "illumos", target_os = "solaris"))]
            BindAddr::Interface(iface) => match family {
                AddressFamily::Ipv4 => {
                    socket.bind_device_by_index_v4(Some(iface.id()))?;
                    IpAddr::V4(Ipv4Addr::UNSPECIFIED)
                }
                AddressFamily::Ipv6 => {
                    socket.bind_device_by_index_v6(Some(iface.id()))?;
                    IpAddr::V6(Ipv6Addr::UNSPECIFIED)
                }
            },
            #[cfg(target_os = "linux")]
            BindAddr::Foreign(addr) => {
                if AddressFamily::from(addr) != family {
                    return Err(io::Error::new(
                        io::ErrorKind::InvalidInput,
                        "foreign bind addr has incorrect address family",
                    ));
                }
                match addr {
                    SocketAddr::V4(_) => {
                        socket.set_ip_transparent_v4(true)?;
                    }
                    SocketAddr::V6(_) => {
                        crate::sockopt::set_ip_transparent_v6(socket, true)?;
                    }
                }
                let addr: SockAddr = (*addr).into();
                return socket.bind(&addr);
            }
        };
        let bind_addr = SockAddr::from(SocketAddr::new(bind_ip, 0));
        socket.bind(&bind_addr)
    }
}
