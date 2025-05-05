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

use anyhow::anyhow;
use chrono::{DateTime, Utc};
use http::uri::PathAndQuery;
use http::{HeaderMap, HeaderValue, header};
use itoa::Buffer;

use g3_http::client::HttpForwardRemoteResponse;

use crate::config::exporter::influxdb::{InfluxdbExporterConfig, TimestampPrecision};
use crate::runtime::export::HttpExport;
use crate::types::{MetricRecord, MetricType};

pub(super) struct InfluxdbHttpFormatter {
    api_path: PathAndQuery,
    static_headers: HeaderMap,
    precision: TimestampPrecision,
}

impl InfluxdbHttpFormatter {
    pub(super) fn new(config: &InfluxdbExporterConfig) -> anyhow::Result<Self> {
        let api_path = config.build_api_path()?;
        let mut static_headers = HeaderMap::new();
        static_headers.insert(
            header::CONTENT_TYPE,
            HeaderValue::from_static("text/plain; charset=utf-8"),
        );
        static_headers.insert(header::ACCEPT, HeaderValue::from_static("application/json"));
        // TODO add auth headers, Basic / Token
        Ok(InfluxdbHttpFormatter {
            api_path,
            static_headers,
            precision: config.precision,
        })
    }
}

impl HttpExport for InfluxdbHttpFormatter {
    fn api_path(&self) -> &PathAndQuery {
        &self.api_path
    }

    fn static_headers(&self) -> &HeaderMap {
        &self.static_headers
    }

    fn fill_body(&self, records: &[(DateTime<Utc>, MetricRecord)], body_buf: &mut Vec<u8>) {
        for (time, record) in records {
            let _ = write!(body_buf, "{}", record.name.display('.'));
            if !record.tag_map.is_empty() {
                let _ = write!(body_buf, ",{}", record.tag_map.display_influxdb());
            }

            match record.r#type {
                MetricType::Counter => {
                    let _ = write!(body_buf, " count={}", record.value.display_influxdb());
                }
                MetricType::Gauge => {
                    let _ = write!(body_buf, " value={}", record.value.display_influxdb());
                }
            }

            let mut ts_buffer = Buffer::new();
            match self.precision {
                TimestampPrecision::Seconds => {
                    let ts = ts_buffer.format(time.timestamp());
                    body_buf.push(b' ');
                    body_buf.extend_from_slice(ts.as_bytes());
                }
                TimestampPrecision::MilliSeconds => {
                    let ts = ts_buffer.format(time.timestamp_millis());
                    body_buf.push(b' ');
                    body_buf.extend_from_slice(ts.as_bytes());
                }
                TimestampPrecision::MicroSeconds => {
                    let ts = ts_buffer.format(time.timestamp_micros());
                    body_buf.push(b' ');
                    body_buf.extend_from_slice(ts.as_bytes());
                }
                TimestampPrecision::NanoSeconds => {
                    if let Some(ts_nanos) = time.timestamp_nanos_opt() {
                        let ts = ts_buffer.format(ts_nanos);
                        body_buf.push(b' ');
                        body_buf.extend_from_slice(ts.as_bytes());
                    }
                }
            };
            body_buf.push(b'\n');
        }
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
