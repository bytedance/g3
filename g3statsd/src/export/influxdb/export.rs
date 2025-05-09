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
use anyhow::anyhow;
use chrono::{DateTime, Utc};
use http::uri::PathAndQuery;
use http::{HeaderMap, HeaderValue, header};
use itoa::Buffer;
use tokio::sync::mpsc;

use g3_http::client::HttpForwardRemoteResponse;

use crate::config::exporter::influxdb::{InfluxdbExporterConfig, TimestampPrecision};
use crate::runtime::export::{AggregateExport, CounterStoreValue, GaugeStoreValue, HttpExport};
use crate::types::{MetricName, MetricTagMap, MetricValue};

pub(super) struct InfluxdbEncodedLines {
    len: usize,
    buf: Vec<u8>,
}

pub(super) struct InfluxdbAggregateExport {
    emit_interval: Duration,
    precision: TimestampPrecision,
    max_body_lines: usize,
    prefix: Option<MetricName>,
    lines_sender: mpsc::Sender<InfluxdbEncodedLines>,

    buf: Vec<u8>,
}

impl InfluxdbAggregateExport {
    pub(super) fn new<T: InfluxdbExporterConfig>(
        config: &T,
        lines_sender: mpsc::Sender<InfluxdbEncodedLines>,
    ) -> Self {
        InfluxdbAggregateExport {
            emit_interval: config.emit_interval(),
            precision: config.precision(),
            max_body_lines: config.max_body_lines(),
            prefix: config.prefix(),
            lines_sender,
            buf: Vec::new(),
        }
    }

    fn serialize_name_tags(&mut self, name: &MetricName, tag_map: &MetricTagMap) {
        if let Some(prefix) = &self.prefix {
            let _ = write!(
                &mut self.buf,
                "{}.{}",
                prefix.display('.'),
                name.display('.')
            );
        } else {
            let _ = write!(&mut self.buf, "{}", name.display('.'));
        }
        if !tag_map.is_empty() {
            let _ = write!(&mut self.buf, ",{}", tag_map.display_influxdb());
        }
    }

    fn serialize_timestamp(&mut self, time: &DateTime<Utc>) {
        let mut ts_buffer = Buffer::new();
        match self.precision {
            TimestampPrecision::Seconds => {
                let ts = ts_buffer.format(time.timestamp());
                self.buf.push(b' ');
                self.buf.extend_from_slice(ts.as_bytes());
            }
            TimestampPrecision::MilliSeconds => {
                let ts = ts_buffer.format(time.timestamp_millis());
                self.buf.push(b' ');
                self.buf.extend_from_slice(ts.as_bytes());
            }
            TimestampPrecision::MicroSeconds => {
                let ts = ts_buffer.format(time.timestamp_micros());
                self.buf.push(b' ');
                self.buf.extend_from_slice(ts.as_bytes());
            }
            TimestampPrecision::NanoSeconds => {
                if let Some(ts_nanos) = time.timestamp_nanos_opt() {
                    let ts = ts_buffer.format(ts_nanos);
                    self.buf.push(b' ');
                    self.buf.extend_from_slice(ts.as_bytes());
                }
            }
        };
    }

    async fn send_lines(&mut self, line_number: usize) {
        if line_number == 0 || self.buf.is_empty() {
            return;
        }
        let _ = self
            .lines_sender
            .send(InfluxdbEncodedLines {
                len: line_number,
                buf: self.buf.clone(),
            })
            .await;
        self.buf.clear();
    }
}

impl AggregateExport for InfluxdbAggregateExport {
    fn emit_interval(&self) -> Duration {
        self.emit_interval
    }

    async fn emit_gauge(
        &mut self,
        name: &MetricName,
        values: &AHashMap<Arc<MetricTagMap>, GaugeStoreValue>,
    ) {
        let mut line_number = 0;
        self.buf.clear();

        for (tag_map, gauge) in values {
            self.serialize_name_tags(name, tag_map);

            let _ = write!(&mut self.buf, " value={}", gauge.value.display_influxdb());

            self.serialize_timestamp(&gauge.time);
            self.buf.push(b'\n');

            line_number += 1;
            if line_number >= self.max_body_lines {
                self.send_lines(line_number).await;
                line_number = 0;
            }
        }

        self.send_lines(line_number).await;
    }

    async fn emit_counter(
        &mut self,
        name: &MetricName,
        values: &AHashMap<Arc<MetricTagMap>, CounterStoreValue>,
    ) {
        let mut line_number = 0;
        self.buf.clear();

        for (tag_map, gauge) in values {
            self.serialize_name_tags(name, tag_map);

            let rate = MetricValue::Double(gauge.diff.as_f64() / self.emit_interval.as_secs_f64());
            let _ = write!(
                &mut self.buf,
                " count={},diff={},rate={}",
                gauge.sum.display_influxdb(),
                gauge.diff.display_influxdb(),
                rate.display_influxdb(),
            );

            self.serialize_timestamp(&gauge.time);
            self.buf.push(b'\n');

            line_number += 1;
            if line_number >= self.max_body_lines {
                self.send_lines(line_number).await;
                line_number = 0;
            }
        }

        self.send_lines(line_number).await;
    }
}

pub(super) struct InfluxdbHttpExport {
    api_path: PathAndQuery,
    static_headers: HeaderMap,
    max_body_lines: usize,
}

impl InfluxdbHttpExport {
    pub(super) fn new<T: InfluxdbExporterConfig>(config: &T) -> anyhow::Result<Self> {
        let api_path = config.build_api_path()?;
        let mut static_headers = HeaderMap::new();
        static_headers.insert(
            header::CONTENT_TYPE,
            HeaderValue::from_static("text/plain; charset=utf-8"),
        );
        static_headers.insert(header::ACCEPT, HeaderValue::from_static("application/json"));
        if let Some(v) = config.build_api_token() {
            static_headers.insert(header::AUTHORIZATION, v);
        }
        Ok(InfluxdbHttpExport {
            api_path,
            static_headers,
            max_body_lines: config.max_body_lines(),
        })
    }
}

// https://docs.influxdata.com/influxdb3/core/write-data/api-client-libraries/
impl HttpExport for InfluxdbHttpExport {
    type BodyPiece = InfluxdbEncodedLines;

    fn api_path(&self) -> &PathAndQuery {
        &self.api_path
    }

    fn static_headers(&self) -> &HeaderMap {
        &self.static_headers
    }

    fn fill_body(&mut self, pieces: &[InfluxdbEncodedLines], body_buf: &mut Vec<u8>) -> usize {
        let mut added_lines = 0;
        let mut handled_pieces = 0;
        for piece in pieces {
            if added_lines + piece.len > self.max_body_lines {
                return handled_pieces;
            }

            body_buf.extend_from_slice(&piece.buf);
            handled_pieces += 1;
            added_lines += piece.len;
        }
        handled_pieces
    }

    fn check_response(&self, rsp: HttpForwardRemoteResponse, body: &[u8]) -> anyhow::Result<()> {
        if rsp.code != 200 && rsp.code != 204 {
            if let Ok(detail) = std::str::from_utf8(body) {
                Err(anyhow!("error response: {} {detail}", rsp.code))
            } else {
                Err(anyhow!("error response: {}", rsp.code))
            }
        } else {
            Ok(())
        }
    }
}
