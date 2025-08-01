/*
 * SPDX-License-Identifier: Apache-2.0
 * Copyright 2024-2025 ByteDance and/or its affiliates.
 */

use std::io;
use std::net::SocketAddr;

use socket2::Socket;

use g3_types::net::{SocketBufferConfig, TcpMiscSockOpts, UdpMiscSockOpts};

use crate::util::AddressFamily;

#[cfg(unix)]
mod unix;
#[cfg(windows)]
mod windows;

#[derive(Debug)]
pub struct RawSocket {
    inner: Option<Socket>,
}

impl RawSocket {
    fn get_inner(&self) -> io::Result<&Socket> {
        self.inner
            .as_ref()
            .ok_or_else(|| io::Error::other("no socket set"))
    }

    pub fn set_buf_opts(&self, buf_conf: SocketBufferConfig) -> io::Result<()> {
        let socket = self.get_inner()?;
        if let Some(size) = buf_conf.recv_size() {
            socket.set_recv_buffer_size(size)?;
        }
        if let Some(size) = buf_conf.send_size() {
            socket.set_send_buffer_size(size)?;
        }
        Ok(())
    }

    pub fn set_tcp_misc_opts(
        &self,
        family: AddressFamily,
        misc_opts: &TcpMiscSockOpts,
        default_set_nodelay: bool,
    ) -> io::Result<()> {
        let socket = self.get_inner()?;
        if let Some(no_delay) = misc_opts.no_delay {
            socket.set_tcp_nodelay(no_delay)?;
        } else if default_set_nodelay {
            socket.set_tcp_nodelay(true)?;
        }
        #[cfg(unix)]
        if let Some(mss) = misc_opts.max_segment_size {
            socket.set_tcp_mss(mss)?;
        }
        match family {
            AddressFamily::Ipv4 => {
                if let Some(ttl) = misc_opts.time_to_live {
                    socket.set_ttl_v4(ttl)?;
                }
                if let Some(tos) = misc_opts.type_of_service {
                    crate::sockopt::set_tos_v4(socket, tos)?;
                }
            }
            AddressFamily::Ipv6 => {
                if let Some(hops) = misc_opts.hop_limit {
                    socket.set_unicast_hops_v6(hops)?;
                }
                #[cfg(not(windows))]
                if let Some(class) = misc_opts.traffic_class {
                    crate::sockopt::set_tclass_v6(socket, class)?;
                }
            }
        }
        #[cfg(any(
            target_os = "linux",
            target_os = "freebsd",
            target_os = "solaris",
            target_os = "illumos"
        ))]
        if let Some(ca) = misc_opts.congestion_control() {
            crate::sockopt::set_tcp_congestion(socket, ca)?;
        }
        #[cfg(target_os = "linux")]
        if let Some(mark) = misc_opts.netfilter_mark {
            socket.set_mark(mark)?;
        }
        Ok(())
    }

    #[cfg(any(target_os = "linux", target_os = "android", target_os = "illumos"))]
    pub fn trigger_tcp_quick_ack(&self) -> io::Result<()> {
        let socket = self.get_inner()?;
        crate::sockopt::set_tcp_quick_ack(socket, true)
    }

    #[cfg(any(target_os = "linux", target_os = "android"))]
    pub fn tcp_incoming_cpu(&self) -> io::Result<usize> {
        let socket = self.get_inner()?;
        super::sockopt::get_incoming_cpu(socket)
    }

    pub fn set_udp_misc_opts(
        &self,
        local_addr: SocketAddr,
        misc_opts: UdpMiscSockOpts,
    ) -> io::Result<()> {
        let socket = self.get_inner()?;
        match local_addr {
            SocketAddr::V4(_) => {
                if let Some(ttl) = misc_opts.time_to_live {
                    socket.set_ttl_v4(ttl)?;
                }
                if let Some(tos) = misc_opts.type_of_service {
                    crate::sockopt::set_tos_v4(socket, tos)?;
                }
            }
            SocketAddr::V6(s6) => {
                if let Some(hops) = misc_opts.hop_limit {
                    socket.set_unicast_hops_v6(hops)?;
                }
                #[cfg(not(windows))]
                if let Some(class) = misc_opts.traffic_class {
                    crate::sockopt::set_tclass_v6(socket, class)?;
                }
                #[cfg(not(target_os = "openbsd"))]
                if s6.ip().is_unspecified()
                    && (misc_opts.time_to_live.is_some() || misc_opts.type_of_service.is_some())
                {
                    // workaround for https://github.com/rust-lang/socket2/pull/603
                    let v6only = super::sockopt::ipv6_only(socket)?;
                    if !v6only {
                        if let Some(ttl) = misc_opts.time_to_live {
                            socket.set_ttl_v4(ttl)?;
                        }
                        if let Some(tos) = misc_opts.type_of_service {
                            crate::sockopt::set_tos_v4(socket, tos)?;
                        }
                    }
                }
            }
        }

        #[cfg(target_os = "linux")]
        if let Some(mark) = misc_opts.netfilter_mark {
            socket.set_mark(mark)?;
        }
        Ok(())
    }
}
