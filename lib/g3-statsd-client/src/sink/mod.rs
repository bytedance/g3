/*
 * SPDX-License-Identifier: Apache-2.0
 * Copyright 2023-2025 ByteDance and/or its affiliates.
 */

use std::io;
use std::net::{SocketAddr, UdpSocket};
#[cfg(unix)]
use std::os::unix::net::UnixDatagram;
#[cfg(unix)]
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

#[cfg(unix)]
mod unix;
#[cfg(unix)]
use unix::UnixMetricsSink;

enum MetricsSinkIo {
    #[cfg(test)]
    Buf(BufMetricsSink),
    Udp(UdpMetricsSink),
    #[cfg(unix)]
    Unix(UnixMetricsSink),
}

impl MetricsSinkIo {
    fn send_msg(&self, buf: &[u8]) -> io::Result<usize> {
        match self {
            #[cfg(test)]
            MetricsSinkIo::Buf(b) => b.send_msg(buf),
            MetricsSinkIo::Udp(s) => s.send_msg(buf),
            #[cfg(unix)]
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

    #[cfg(unix)]
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
