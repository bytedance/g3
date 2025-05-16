/*
 * SPDX-License-Identifier: Apache-2.0
 * Copyright 2023-2025 ByteDance and/or its affiliates.
 */

use std::io;
use std::rc::Rc;
use std::sync::Mutex;

pub(super) struct BufMetricsSink {
    buf: Rc<Mutex<Vec<u8>>>,
}

impl BufMetricsSink {
    pub(super) fn new(buf: Rc<Mutex<Vec<u8>>>) -> Self {
        BufMetricsSink { buf }
    }

    pub(super) fn send_msg(&self, msg: &[u8]) -> io::Result<usize> {
        let mut buf = self.buf.lock().unwrap();
        buf.extend_from_slice(msg);
        Ok(msg.len())
    }
}
