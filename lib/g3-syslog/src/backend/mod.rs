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
use std::net::{IpAddr, SocketAddr, UdpSocket};
#[cfg(unix)]
use std::os::unix::net::UnixDatagram;
#[cfg(unix)]
use std::path::PathBuf;

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
        false
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
