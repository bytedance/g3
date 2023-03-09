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
use std::os::unix::net::UnixDatagram;
use std::path::PathBuf;

mod udp;
mod unix_datagram;

pub(super) enum SyslogBackend {
    Udp(UdpSocket),
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
            SyslogBackend::Unix(s) => s.send(buf),
        }
    }

    fn flush(&mut self) -> io::Result<()> {
        Ok(())
    }
}

#[derive(Clone, Debug)]
pub enum SyslogBackendBuilder {
    Default,
    /// unix socket with path
    Unix(PathBuf),
    /// udp socket with optional bind ip and remote address
    Udp(Option<IpAddr>, SocketAddr),
}

impl SyslogBackendBuilder {
    pub(super) fn build(&self) -> io::Result<SyslogBackend> {
        match self {
            SyslogBackendBuilder::Default => {
                let socket = unix_datagram::default()?;
                Ok(SyslogBackend::Unix(socket))
            }
            SyslogBackendBuilder::Unix(path) => {
                let socket = unix_datagram::custom(path)?;
                Ok(SyslogBackend::Unix(socket))
            }
            SyslogBackendBuilder::Udp(bind_ip, server) => {
                let socket = udp::udp(*bind_ip, *server)?;
                Ok(SyslogBackend::Udp(socket))
            }
        }
    }
}
