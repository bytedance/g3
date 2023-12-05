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
use std::net::{SocketAddr, UdpSocket};
use std::os::unix::net::UnixDatagram;
use std::path::PathBuf;
#[cfg(test)]
use std::rc::Rc;
#[cfg(test)]
use std::sync::Mutex;

#[cfg(test)]
mod buf;
#[cfg(test)]
use buf::BufMetricsSink;

mod udp;
use udp::UdpMetricsSink;

mod unix;
use unix::UnixMetricsSink;

enum MetricsSinkIo {
    #[cfg(test)]
    Buf(BufMetricsSink),
    Udp(UdpMetricsSink),
    Unix(UnixMetricsSink),
}

impl MetricsSinkIo {
    fn send_msg(&self, buf: &[u8]) -> io::Result<usize> {
        match self {
            #[cfg(test)]
            MetricsSinkIo::Buf(b) => b.send_msg(buf),
            MetricsSinkIo::Udp(s) => s.send_msg(buf),
            MetricsSinkIo::Unix(s) => s.send_msg(buf),
        }
    }
}

pub(crate) struct StatsdMetricsSink {
    cache_size: usize,
    buf: Vec<u8>,
    io: MetricsSinkIo,
}

impl StatsdMetricsSink {
    #[cfg(test)]
    pub(crate) fn buf_with_capacity(buf: Rc<Mutex<Vec<u8>>>, cache_size: usize) -> Self {
        StatsdMetricsSink {
            cache_size,
            buf: Vec::with_capacity(cache_size),
            io: MetricsSinkIo::Buf(BufMetricsSink::new(buf)),
        }
    }

    pub(crate) fn udp_with_capacity(
        addr: SocketAddr,
        socket: UdpSocket,
        cache_size: usize,
    ) -> Self {
        StatsdMetricsSink {
            cache_size,
            buf: Vec::with_capacity(cache_size),
            io: MetricsSinkIo::Udp(UdpMetricsSink::new(addr, socket)),
        }
    }

    pub(crate) fn unix_with_capacity(
        path: PathBuf,
        socket: UnixDatagram,
        cache_size: usize,
    ) -> Self {
        StatsdMetricsSink {
            cache_size,
            buf: Vec::with_capacity(cache_size),
            io: MetricsSinkIo::Unix(UnixMetricsSink::new(path, socket)),
        }
    }

    pub(super) fn emit<F>(&mut self, msg_len: usize, format: F) -> io::Result<()>
    where
        F: Fn(&mut Vec<u8>),
    {
        if self.buf.is_empty() {
            format(&mut self.buf);
        } else if self.buf.len() + 1 + msg_len > self.cache_size {
            self.flush_buf()?;
            format(&mut self.buf);
        } else {
            self.buf.push(b'\n');
            format(&mut self.buf);
        }
        Ok(())
    }

    pub(super) fn flush(&mut self) -> io::Result<()> {
        if self.buf.is_empty() {
            return Ok(());
        }
        self.flush_buf()
    }

    fn flush_buf(&mut self) -> io::Result<()> {
        self.io.send_msg(&self.buf)?;
        self.buf.clear();
        Ok(())
    }
}
