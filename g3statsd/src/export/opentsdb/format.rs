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
use serde_json::{Map, Number, Value};

use g3_http::client::HttpForwardRemoteResponse;

use crate::config::exporter::opentsdb::OpentsdbExporterConfig;
use crate::runtime::export::HttpExport;
use crate::types::MetricRecord;

pub(super) struct OpentsdbHttpFormatter {
    api_path: PathAndQuery,
    static_headers: HeaderMap,
}

impl OpentsdbHttpFormatter {
    pub(super) fn new(config: &OpentsdbExporterConfig) -> anyhow::Result<Self> {
        let api_path = config.build_api_path()?;
        let mut static_headers = HeaderMap::new();
        static_headers.insert(
            header::CONTENT_TYPE,
            HeaderValue::from_static("application/json"),
        );
        static_headers.insert(header::ACCEPT, HeaderValue::from_static("application/json"));
        // TODO add auth headers, Basic / Token
        Ok(OpentsdbHttpFormatter {
            api_path,
            static_headers,
        })
    }
}

fn record_to_json(time: &DateTime<Utc>, record: &MetricRecord) -> Value {
    let mut map = Map::with_capacity(4);
    map.insert(
        "metric".to_string(),
        Value::String(record.name.display('.').to_string()),
    );
    map.insert(
        "timestamp".to_string(),
        Value::Number(Number::from(time.timestamp())),
    );
    map.insert("value".to_string(), Value::Number(record.value.into()));
    let mut tag_map = Map::with_capacity(record.tag_map.len());
    for (name, value) in record.tag_map.iter() {
        tag_map.insert(name.to_string(), Value::String(value.to_string()));
    }
    map.insert("tags".to_string(), Value::Object(tag_map));
    Value::Object(map)
}

impl HttpExport for OpentsdbHttpFormatter {
    fn api_path(&self) -> &PathAndQuery {
        &self.api_path
    }

    fn static_headers(&self) -> &HeaderMap {
        &self.static_headers
    }

    fn fill_body(&self, records: &[(DateTime<Utc>, MetricRecord)], body_buf: &mut Vec<u8>) {
        body_buf.push(b'[');

        let mut iter = records.iter();
        let Some((time, record)) = iter.next() else {
            body_buf.push(b']');
            return;
        };

        let first_v = record_to_json(time, record);
        let _ = write!(body_buf, "{first_v}");

        for (time, record) in iter {
            body_buf.push(b',');
            let v = record_to_json(time, record);
            let _ = write!(body_buf, "{v}");
        }

        body_buf.push(b']');
    }

    fn check_response(&self, rsp: HttpForwardRemoteResponse, body: &[u8]) -> anyhow::Result<()> {
        if rsp.code != 204 {
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
