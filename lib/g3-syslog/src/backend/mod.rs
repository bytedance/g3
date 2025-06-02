/*
 * SPDX-License-Identifier: Apache-2.0
 * Copyright 2023-2025 ByteDance and/or its affiliates.
 */

use std::io;
use std::net::{IpAddr, SocketAddr, UdpSocket};
#[cfg(unix)]
use std::os::unix::net::UnixDatagram;
#[cfg(unix)]
use std::path::PathBuf;

#[cfg(feature = "yaml")]
mod yaml;

mod udp;
#[cfg(unix)]
mod unix_datagram;

pub(super) enum SyslogBackend {
    Udp(UdpSocket),
    #[cfg(unix)]
    Unix(UnixDatagram),
}

impl SyslogBackend {
    pub(super) fn need_reconnect(&self) -> bool {
        match self {
            SyslogBackend::Udp(_) => false,
            #[cfg(unix)]
            SyslogBackend::Unix(_) => true,
        }
    }
}

impl io::Write for SyslogBackend {
    fn write(&mut self, buf: &[u8]) -> io::Result<usize> {
        match self {
            SyslogBackend::Udp(s) => s.send(buf),
            #[cfg(unix)]
            SyslogBackend::Unix(s) => s.send(buf),
        }
    }

    fn flush(&mut self) -> io::Result<()> {
        Ok(())
    }
}

#[derive(Clone, Debug)]
pub enum SyslogBackendBuilder {
    /// unix socket with path
    #[cfg(unix)]
    Unix(Option<PathBuf>),
    /// udp socket with optional bind ip and remote address
    Udp(Option<IpAddr>, SocketAddr),
}

#[cfg(unix)]
impl Default for SyslogBackendBuilder {
    fn default() -> Self {
        SyslogBackendBuilder::Unix(None)
    }
}

#[cfg(not(unix))]
impl Default for SyslogBackendBuilder {
    fn default() -> Self {
        use std::net::Ipv4Addr;

        SyslogBackendBuilder::Udp(None, SocketAddr::new(IpAddr::V4(Ipv4Addr::LOCALHOST), 514))
    }
}

impl SyslogBackendBuilder {
    pub(super) fn build(&self) -> io::Result<SyslogBackend> {
        match self {
            #[cfg(unix)]
            SyslogBackendBuilder::Unix(path) => {
                let socket = if let Some(path) = path {
                    unix_datagram::custom(path)?
                } else {
                    unix_datagram::default()?
                };
                Ok(SyslogBackend::Unix(socket))
            }
            SyslogBackendBuilder::Udp(bind_ip, server) => {
                let socket = udp::udp(*bind_ip, *server)?;
                Ok(SyslogBackend::Udp(socket))
            }
        }
    }
}
