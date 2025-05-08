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
use serde_json::{Map, Number, Value};
use tokio::sync::mpsc;

use g3_http::client::HttpForwardRemoteResponse;

use crate::config::exporter::opentsdb::OpentsdbExporterConfig;
use crate::runtime::export::{AggregateExport, CounterStoreValue, GaugeStoreValue, HttpExport};
use crate::types::{MetricName, MetricTagMap, MetricValue};

pub(super) struct OpentsdbAggregateExport {
    emit_interval: Duration,
    max_data_points: usize,
    lines_sender: mpsc::Sender<Vec<Value>>,
}

impl OpentsdbAggregateExport {
    pub(super) fn new(
        config: &OpentsdbExporterConfig,
        lines_sender: mpsc::Sender<Vec<Value>>,
    ) -> Self {
        OpentsdbAggregateExport {
            emit_interval: config.emit_interval,
            max_data_points: config.max_data_points,
            lines_sender,
        }
    }

    fn build_data_point(
        name: &MetricName,
        time: &DateTime<Utc>,
        tags: &MetricTagMap,
        value: &MetricValue,
    ) -> Value {
        let mut map = Map::with_capacity(4);
        map.insert(
            "metric".to_string(),
            Value::String(name.display('.').to_string()),
        );
        map.insert(
            "timestamp".to_string(),
            Value::Number(Number::from(time.timestamp())),
        );
        map.insert("value".to_string(), Value::Number(value.as_json_number()));
        let mut tag_map = Map::with_capacity(tags.len());
        for (name, value) in tags.iter() {
            tag_map.insert(name.to_string(), Value::String(value.to_string()));
        }
        map.insert("tags".to_string(), Value::Object(tag_map));
        Value::Object(map)
    }
}

impl AggregateExport for OpentsdbAggregateExport {
    fn emit_interval(&self) -> Duration {
        self.emit_interval
    }

    async fn emit_gauge(
        &mut self,
        name: &MetricName,
        values: &AHashMap<Arc<MetricTagMap>, GaugeStoreValue>,
    ) {
        let mut data_points = Vec::with_capacity(self.max_data_points);
        for (tag_map, v) in values {
            if data_points.len() >= self.max_data_points {
                let _ = self.lines_sender.send(data_points).await;
                data_points = Vec::with_capacity(self.max_data_points);
            }
            let data = Self::build_data_point(name, &v.time, tag_map, &v.value);
            data_points.push(data);
        }
    }

    async fn emit_counter(
        &mut self,
        name: &MetricName,
        values: &AHashMap<Arc<MetricTagMap>, CounterStoreValue>,
    ) {
        let mut data_points = Vec::with_capacity(self.max_data_points);
        for (tag_map, v) in values {
            if data_points.len() >= self.max_data_points {
                let _ = self.lines_sender.send(data_points).await;
                data_points = Vec::with_capacity(self.max_data_points);
            }
            let data = Self::build_data_point(name, &v.time, tag_map, &v.sum);
            data_points.push(data);
        }
    }
}

pub(super) struct OpentsdbHttpExport {
    api_path: PathAndQuery,
    static_headers: HeaderMap,
    max_data_points: usize,
}

impl OpentsdbHttpExport {
    pub(super) fn new(config: &OpentsdbExporterConfig) -> anyhow::Result<Self> {
        let api_path = config.build_api_path()?;
        let mut static_headers = HeaderMap::new();
        static_headers.insert(
            header::CONTENT_TYPE,
            HeaderValue::from_static("application/json"),
        );
        static_headers.insert(header::ACCEPT, HeaderValue::from_static("application/json"));
        // TODO add auth headers, Basic / Token
        Ok(OpentsdbHttpExport {
            api_path,
            static_headers,
            max_data_points: config.max_data_points,
        })
    }
}

// https://opentsdb.net/docs/build/html/api_http/put.html
impl HttpExport for OpentsdbHttpExport {
    type BodyPiece = Vec<Value>;

    fn api_path(&self) -> &PathAndQuery {
        &self.api_path
    }

    fn static_headers(&self) -> &HeaderMap {
        &self.static_headers
    }

    fn fill_body(&mut self, pieces: &[Vec<Value>], body_buf: &mut Vec<u8>) -> usize {
        let mut added_data_points = 0;
        let mut handled_pieces = 0;
        body_buf.push(b'[');

        for piece in pieces {
            if added_data_points + piece.len() > self.max_data_points {
                break;
            }
            handled_pieces += 1;

            let mut iter = piece.iter();
            if added_data_points == 0 {
                let Some(first_v) = iter.next() else {
                    continue;
                };
                let _ = write!(body_buf, "{first_v}");
                added_data_points += 1;
            }

            for v in iter {
                let _ = write!(body_buf, ",{v}");
                added_data_points += 1;
            }
        }

        body_buf.push(b']');
        handled_pieces
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
