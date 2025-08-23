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

mod buf;
use buf::SinkBuf;

#[cfg(test)]
mod test;
#[cfg(test)]
use test::TestMetricsSink;

mod udp;
use udp::UdpMetricsSink;

#[cfg(unix)]
mod unix;
#[cfg(unix)]
use unix::UnixMetricsSink;

enum MetricsSinkIo {
    #[cfg(test)]
    Buf(TestMetricsSink),
    Udp(UdpMetricsSink),
    #[cfg(unix)]
    Unix(UnixMetricsSink),
}

impl MetricsSinkIo {
    fn send_batch(&self, buf: &mut SinkBuf) -> io::Result<()> {
        match self {
            #[cfg(test)]
            MetricsSinkIo::Buf(b) => {
                b.send_batch(buf);
                Ok(())
            }
            MetricsSinkIo::Udp(s) => s.send_batch(buf),
            #[cfg(unix)]
            MetricsSinkIo::Unix(s) => s.send_batch(buf),
        }
    }
}

pub(crate) struct StatsdMetricsSink {
    buf: SinkBuf,
    io: MetricsSinkIo,
}

impl StatsdMetricsSink {
    #[cfg(test)]
    pub(crate) fn test_with_capacity(buf: Rc<Mutex<Vec<u8>>>, cache_size: usize) -> Self {
        StatsdMetricsSink {
            buf: SinkBuf::new(cache_size),
            io: MetricsSinkIo::Buf(TestMetricsSink::new(buf)),
        }
    }

    pub(crate) fn udp_with_capacity(
        addr: SocketAddr,
        socket: UdpSocket,
        cache_size: usize,
        max_segment_size: Option<usize>,
    ) -> Self {
        StatsdMetricsSink {
            buf: SinkBuf::new(cache_size),
            io: MetricsSinkIo::Udp(UdpMetricsSink::new(addr, socket, max_segment_size)),
        }
    }

    #[cfg(unix)]
    pub(crate) fn unix_with_capacity(
        path: PathBuf,
        socket: UnixDatagram,
        cache_size: usize,
        max_segment_size: Option<usize>,
    ) -> Self {
        StatsdMetricsSink {
            buf: SinkBuf::new(cache_size),
            io: MetricsSinkIo::Unix(UnixMetricsSink::new(path, socket, max_segment_size)),
        }
    }

    pub(super) fn emit<F>(&mut self, format: F) -> io::Result<()>
    where
        F: Fn(&mut Vec<u8>),
    {
        self.buf.receive(format);
        if self.buf.buf_full() {
            self.flush_buf()?;
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
        let r = self.io.send_batch(&mut self.buf);
        self.buf.reset();
        r
    }
}
