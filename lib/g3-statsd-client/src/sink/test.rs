/*
 * SPDX-License-Identifier: Apache-2.0
 * Copyright 2023-2025 ByteDance and/or its affiliates.
 */

use std::rc::Rc;
use std::sync::Mutex;

use super::SinkBuf;

pub(super) struct TestMetricsSink {
    buf: Rc<Mutex<Vec<u8>>>,
}

impl TestMetricsSink {
    pub(super) fn new(buf: Rc<Mutex<Vec<u8>>>) -> Self {
        TestMetricsSink { buf }
    }

    pub(super) fn send_batch(&self, buf: &mut SinkBuf) {
        for packet in buf.iter(4096) {
            let mut buf = self.buf.lock().unwrap();
            buf.extend_from_slice(packet.as_ref());
        }
    }
}
