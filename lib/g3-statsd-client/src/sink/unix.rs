/*
 * SPDX-License-Identifier: Apache-2.0
 * Copyright 2023-2025 ByteDance and/or its affiliates.
 */

use std::io;
use std::os::unix::net::UnixDatagram;
use std::path::PathBuf;

use super::SinkBuf;

pub(super) struct UnixMetricsSink {
    path: PathBuf,
    socket: UnixDatagram,
    max_segment_size: usize,
}

impl UnixMetricsSink {
    pub(super) fn new(
        path: PathBuf,
        socket: UnixDatagram,
        max_segment_size: Option<usize>,
    ) -> Self {
        UnixMetricsSink {
            path,
            socket,
            max_segment_size: max_segment_size.unwrap_or(4096),
        }
    }

    pub(super) fn send_batch(&self, buf: &mut SinkBuf) -> io::Result<()> {
        for packet in buf.iter(self.max_segment_size) {
            self.socket.send_to(packet.as_ref(), &self.path)?;
        }
        Ok(())
    }
}
