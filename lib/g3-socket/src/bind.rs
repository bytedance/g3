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
}

impl BindAddr {
    pub fn is_none(&self) -> bool {
        matches!(self, BindAddr::None)
    }

    pub fn ip(&self) -> Option<IpAddr> {
        if let BindAddr::Ip(ip) = self {
            Some(*ip)
        } else {
            None
        }
    }

    pub(crate) fn bind_for_connect(
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
                let _ = set_reuse_unicastport(socket, true);
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
        };
        let bind_addr = SockAddr::from(SocketAddr::new(bind_ip, 0));
        socket.bind(&bind_addr)
    }
}
