/*
 * SPDX-License-Identifier: Apache-2.0
 * Copyright 2023-2025 ByteDance and/or its affiliates.
 */

use std::io;
use std::os::unix::net::UnixDatagram;
use std::path::PathBuf;

pub(super) struct UnixMetricsSink {
    path: PathBuf,
    socket: UnixDatagram,
}

impl UnixMetricsSink {
    pub(super) fn new(path: PathBuf, socket: UnixDatagram) -> Self {
        UnixMetricsSink { path, socket }
    }

    pub(super) fn send_msg(&self, msg: &[u8]) -> io::Result<usize> {
        self.socket.send_to(msg, &self.path)
    }
}
