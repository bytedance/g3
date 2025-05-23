/*
 * SPDX-License-Identifier: Apache-2.0
 * Copyright 2024-2025 ByteDance and/or its affiliates.
 */

use std::io;

use socket2::Socket;

use g3_types::net::{SocketBufferConfig, TcpMiscSockOpts, UdpMiscSockOpts};

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
        misc_opts: &TcpMiscSockOpts,
        default_set_nodelay: bool,
    ) -> io::Result<()> {
        let socket = self.get_inner()?;
        if let Some(no_delay) = misc_opts.no_delay {
            socket.set_nodelay(no_delay)?;
        } else if default_set_nodelay {
            socket.set_nodelay(true)?;
        }
        #[cfg(unix)]
        if let Some(mss) = misc_opts.max_segment_size {
            socket.set_mss(mss)?;
        }
        if let Some(ttl) = misc_opts.time_to_live {
            socket.set_ttl(ttl)?;
        }
        #[cfg(not(target_os = "illumos"))]
        if let Some(tos) = misc_opts.type_of_service {
            socket.set_tos(tos as u32)?;
        }
        #[cfg(target_os = "linux")]
        if let Some(mark) = misc_opts.netfilter_mark {
            socket.set_mark(mark)?;
        }
        Ok(())
    }

    #[cfg(any(target_os = "linux", target_os = "android"))]
    pub fn trigger_tcp_quick_ack(&self) -> io::Result<()> {
        let socket = self.get_inner()?;
        socket.set_quickack(true)
    }

    #[cfg(any(target_os = "linux", target_os = "android"))]
    pub fn tcp_incoming_cpu(&self) -> io::Result<usize> {
        let socket = self.get_inner()?;
        super::sockopt::get_incoming_cpu(socket)
    }

    pub fn set_udp_misc_opts(&self, misc_opts: UdpMiscSockOpts) -> io::Result<()> {
        let socket = self.get_inner()?;
        if let Some(ttl) = misc_opts.time_to_live {
            socket.set_ttl(ttl)?;
        }
        #[cfg(not(target_os = "illumos"))]
        if let Some(tos) = misc_opts.type_of_service {
            socket.set_tos(tos as u32)?;
        }
        #[cfg(target_os = "linux")]
        if let Some(mark) = misc_opts.netfilter_mark {
            socket.set_mark(mark)?;
        }
        Ok(())
    }
}
