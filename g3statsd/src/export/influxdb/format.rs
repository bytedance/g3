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
use http::{HeaderMap, HeaderValue, header};

use g3_http::client::HttpForwardRemoteResponse;

use crate::runtime::export::HttpExport;
use crate::types::{MetricRecord, MetricType};

#[derive(Default)]
pub(super) struct InfluxdbHttpFormatter {
    close_connection: bool,
}

impl HttpExport for InfluxdbHttpFormatter {
    fn serialize(
        &mut self,
        records: &[(DateTime<Utc>, MetricRecord)],
        headers: &mut HeaderMap,
        body_buf: &mut Vec<u8>,
    ) {
        self.close_connection = true;

        headers.insert(
            header::CONTENT_TYPE,
            HeaderValue::from_static("text/plain; charset=utf-8"),
        );
        headers.insert(header::ACCEPT, HeaderValue::from_static("application/json"));

        // TODO add auth headers, Basic / Token

        for (time, record) in records {
            let timestamp_ns = time.timestamp_nanos_opt().unwrap();

            match record.r#type {
                MetricType::Counter => {
                    let _ = writeln!(
                        body_buf,
                        "{},{} count={} {timestamp_ns}",
                        record.name.display('.'),
                        record.tag_map.display_influxdb(),
                        record.value.display_influxdb(),
                    );
                }
                MetricType::Gauge => {
                    let _ = writeln!(
                        body_buf,
                        "{},{} value={} {timestamp_ns}",
                        record.name.display('.'),
                        record.tag_map.display_influxdb(),
                        record.value.display_influxdb(),
                    );
                }
            }
        }

        self.close_connection = false;
    }

    fn check_response(
        &mut self,
        rsp: HttpForwardRemoteResponse,
        body: &[u8],
    ) -> anyhow::Result<()> {
        self.close_connection = true;

        if rsp.code != 200 {
            if let Ok(detail) = std::str::from_utf8(body) {
                Err(anyhow!("Post data failed: {} {detail}", rsp.code))
            } else {
                Err(anyhow!("Post data failed: {}", rsp.code))
            }
        } else {
            self.close_connection = !rsp.keep_alive();
            Ok(())
        }
    }

    fn close_connection(&self) -> bool {
        self.close_connection
    }
}
