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
