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
use std::sync::Arc;
use std::time::Duration;

use ahash::AHashMap;
use chrono::{DateTime, Utc};
use itoa::Buffer;
use tokio::sync::mpsc;

use crate::config::exporter::graphite::GraphiteExporterConfig;
use crate::runtime::export::{AggregateExport, CounterStoreValue, GaugeStoreValue, StreamExport};
use crate::types::{MetricName, MetricTagMap, MetricValue};

pub(super) struct GraphitePlaintextAggregateExport {
    emit_interval: Duration,
    data_sender: mpsc::Sender<Vec<u8>>,

    buf: Vec<u8>,
}

impl GraphitePlaintextAggregateExport {
    pub(super) fn new(config: &GraphiteExporterConfig, data_sender: mpsc::Sender<Vec<u8>>) -> Self {
        GraphitePlaintextAggregateExport {
            emit_interval: config.emit_interval,
            data_sender,
            buf: Vec::with_capacity(2048),
        }
    }

    fn serialize(
        &mut self,
        time: &DateTime<Utc>,
        name: &MetricName,
        tags: &MetricTagMap,
        value: &MetricValue,
    ) {
        let _ = write!(self.buf, "{}", name.display('.'));
        if !tags.is_empty() {
            let _ = write!(self.buf, ";{}", tags.display_graphite());
        }
        let _ = write!(self.buf, " {value}");
        let mut ts_buffer = Buffer::new();
        let ts = ts_buffer.format(time.timestamp());
        self.buf.push(b' ');
        self.buf.extend_from_slice(ts.as_bytes());
        self.buf.push(b'\n');
    }
}

impl AggregateExport for GraphitePlaintextAggregateExport {
    fn emit_interval(&self) -> Duration {
        self.emit_interval
    }

    async fn emit_gauge(
        &mut self,
        name: &MetricName,
        values: &AHashMap<Arc<MetricTagMap>, GaugeStoreValue>,
    ) {
        self.buf.clear();
        let now = Utc::now();
        for (tags, v) in values {
            self.serialize(&now, name, tags, &v.value);
        }
        let _ = self.data_sender.send(self.buf.clone()).await;
    }

    async fn emit_counter(
        &mut self,
        name: &MetricName,
        values: &AHashMap<Arc<MetricTagMap>, CounterStoreValue>,
    ) {
        self.buf.clear();
        let now = Utc::now();
        for (tags, v) in values {
            self.serialize(&now, name, tags, &v.sum);
        }
        let _ = self.data_sender.send(self.buf.clone()).await;
    }
}

#[derive(Default)]
pub(super) struct GraphitePlaintextStreamExport {}

impl StreamExport for GraphitePlaintextStreamExport {
    type Piece = Vec<u8>;

    fn serialize(&self, pieces: &[Vec<u8>], buf: &mut Vec<u8>) -> usize {
        for piece in pieces {
            buf.extend_from_slice(piece.as_slice());
        }
        pieces.len()
    }
}
