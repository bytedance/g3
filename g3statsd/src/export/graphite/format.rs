/*
 * Copyright 2025 ByteDance and/or its affiliates.
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

use std::io::Write;

use chrono::{DateTime, Utc};
use itoa::Buffer;

use crate::runtime::export::StreamExport;
use crate::types::MetricRecord;

#[derive(Default)]
pub(super) struct GraphitePlaintextFormatter {}

impl StreamExport for GraphitePlaintextFormatter {
    fn serialize(&self, time: DateTime<Utc>, record: &MetricRecord, buf: &mut Vec<u8>) {
        let _ = write!(buf, "{}", record.name.display('.'));
        if !record.tag_map.is_empty() {
            let _ = write!(buf, ";{}", record.tag_map.display_graphite());
        }
        let _ = write!(buf, " {}", record.value);
        let mut ts_buffer = Buffer::new();
        let ts = ts_buffer.format(time.timestamp());
        buf.push(b' ');
        buf.extend_from_slice(ts.as_bytes());
        buf.push(b'\n');
    }
}
