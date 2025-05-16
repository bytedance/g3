/*
 * SPDX-License-Identifier: Apache-2.0
 * Copyright 2025 ByteDance and/or its affiliates.
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
use g3_types::metrics::MetricTagMap;

use crate::config::exporter::opentsdb::OpentsdbExporterConfig;
use crate::runtime::export::{AggregateExport, CounterStoreValue, GaugeStoreValue, HttpExport};
use crate::types::{MetricName, MetricValue};

pub(super) struct OpentsdbAggregateExport {
    emit_interval: Duration,
    max_data_points: usize,
    prefix: Option<MetricName>,
    global_tags: MetricTagMap,
    values_sender: mpsc::UnboundedSender<Vec<Value>>,

    value_buf: Vec<Value>,
}

impl OpentsdbAggregateExport {
    pub(super) fn new(
        config: &OpentsdbExporterConfig,
        values_sender: mpsc::UnboundedSender<Vec<Value>>,
    ) -> Self {
        OpentsdbAggregateExport {
            emit_interval: config.emit_interval,
            max_data_points: config.max_data_points,
            prefix: config.prefix.clone(),
            global_tags: config.global_tags.clone(),
            values_sender,
            value_buf: Vec::with_capacity(32),
        }
    }

    fn build_data_point(
        &self,
        name: &MetricName,
        time: &DateTime<Utc>,
        tags: &MetricTagMap,
        value: &MetricValue,
    ) -> Value {
        let mut map = Map::with_capacity(4);
        let name = self
            .prefix
            .as_ref()
            .map(|p| format!("{}.{}", p.display('.'), name.display('.')))
            .unwrap_or_else(|| name.display('.').to_string());
        map.insert("metric".to_string(), Value::String(name));
        map.insert(
            "timestamp".to_string(),
            Value::Number(Number::from(time.timestamp())),
        );
        map.insert("value".to_string(), Value::Number(value.as_json_number()));
        let mut tag_map = Map::with_capacity(tags.len());
        for (name, value) in self.global_tags.iter() {
            tag_map.insert(name.to_string(), Value::String(value.to_string()));
        }
        for (name, value) in tags.iter() {
            tag_map.insert(name.to_string(), Value::String(value.to_string()));
        }
        map.insert("tags".to_string(), Value::Object(tag_map));
        Value::Object(map)
    }

    fn send_data_points(&mut self) {
        if self.value_buf.is_empty() {
            return;
        }
        let new_buf = Vec::with_capacity(self.value_buf.capacity());
        let data_points = std::mem::replace(&mut self.value_buf, new_buf);
        let _ = self.values_sender.send(data_points);
    }
}

impl AggregateExport for OpentsdbAggregateExport {
    fn emit_interval(&self) -> Duration {
        self.emit_interval
    }

    fn emit_gauge(
        &mut self,
        name: &MetricName,
        values: &AHashMap<Arc<MetricTagMap>, GaugeStoreValue>,
    ) {
        self.value_buf.clear();
        for (tag_map, v) in values {
            if self.value_buf.len() >= self.max_data_points {
                self.send_data_points();
            }
            let data = self.build_data_point(name, &v.time, tag_map, &v.value);
            self.value_buf.push(data);
        }
        self.send_data_points();
    }

    fn emit_counter(
        &mut self,
        name: &MetricName,
        values: &AHashMap<Arc<MetricTagMap>, CounterStoreValue>,
    ) {
        self.value_buf.clear();
        for (tag_map, v) in values {
            if self.value_buf.len() >= self.max_data_points {
                self.send_data_points();
            }
            let data = self.build_data_point(name, &v.time, tag_map, &v.sum);
            self.value_buf.push(data);
        }
        self.send_data_points();
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
