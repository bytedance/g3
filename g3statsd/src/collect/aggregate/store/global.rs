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

use std::sync::Arc;

use ahash::AHashMap;
use chrono::Utc;
use tokio::sync::{broadcast, mpsc};

use g3_types::metrics::MetricTagMap;

use super::Command;
use crate::collect::ArcCollector;
use crate::config::collector::aggregate::AggregateCollectorConfig;
use crate::export::ArcExporter;
use crate::types::{MetricName, MetricRecord, MetricType, MetricValue};

const BATCH_SIZE: usize = 128;

pub(super) struct GlobalStore {
    config: Arc<AggregateCollectorConfig>,
    cfg_receiver: broadcast::Receiver<Arc<AggregateCollectorConfig>>,
    cmd_receiver: mpsc::UnboundedReceiver<Command>,

    next: Option<ArcCollector>,
    exporters: Vec<ArcExporter>,

    counter: AHashMap<Arc<MetricName>, AHashMap<Arc<MetricTagMap>, MetricValue>>,
    gauge: AHashMap<Arc<MetricName>, AHashMap<Arc<MetricTagMap>, MetricValue>>,
}

impl GlobalStore {
    pub(super) fn new(
        config: Arc<AggregateCollectorConfig>,
        cfg_receiver: broadcast::Receiver<Arc<AggregateCollectorConfig>>,
        cmd_receiver: mpsc::UnboundedReceiver<Command>,
    ) -> Self {
        let next = config
            .next
            .as_ref()
            .map(|name| crate::collect::get_or_insert_default(name));
        let exporters = config
            .exporters
            .iter()
            .map(crate::export::get_or_insert_default)
            .collect();

        GlobalStore {
            config,
            cfg_receiver,
            cmd_receiver,
            next,
            exporters,
            counter: Default::default(),
            gauge: Default::default(),
        }
    }

    pub(super) async fn into_running(mut self) {
        let mut buffer = Vec::with_capacity(BATCH_SIZE);
        loop {
            tokio::select! {
                biased;

                r = self.cfg_receiver.recv() => {
                    match r {
                        Ok(config) => {
                            self.update_config(config);
                        }
                        Err(broadcast::error::RecvError::Closed) => break,
                        Err(broadcast::error::RecvError::Lagged(_)) => continue,
                    }
                }
                nr = self.cmd_receiver.recv_many(&mut buffer, BATCH_SIZE) => {
                    if nr == 0 {
                        self.emit();
                        return;
                    }
                    self.handle_cmd(&mut buffer);
                }
            }
        }

        loop {
            let nr = self.cmd_receiver.recv_many(&mut buffer, BATCH_SIZE).await;
            if nr == 0 {
                self.emit();
                break;
            }
            self.handle_cmd(&mut buffer);
        }
    }

    fn update_config(&mut self, config: Arc<AggregateCollectorConfig>) {
        self.next = config
            .next
            .as_ref()
            .map(|name| crate::collect::get_or_insert_default(name));
        self.exporters = config
            .exporters
            .iter()
            .map(crate::export::get_or_insert_default)
            .collect();
        self.config = config;
    }

    fn handle_cmd(&mut self, buffer: &mut Vec<Command>) {
        for cmd in buffer.drain(..) {
            match cmd {
                Command::Add(record) => self.add_record(record),
                Command::Sync(_) => unreachable!(),
                Command::Emit => self.emit(),
            }
        }
    }

    fn add_record(&mut self, record: MetricRecord) {
        match record.r#type {
            MetricType::Counter => {
                let MetricRecord {
                    r#type: _,
                    name,
                    tag_map,
                    value,
                } = record;

                self.counter
                    .entry(name)
                    .or_default()
                    .entry(tag_map)
                    .and_modify(|v| *v += value)
                    .or_insert(value);
            }
            MetricType::Gauge => {
                let MetricRecord {
                    r#type: _,
                    name,
                    tag_map,
                    value,
                } = record;

                self.gauge
                    .entry(name)
                    .or_default()
                    .entry(tag_map)
                    .and_modify(|v| *v = value)
                    .or_insert(value);
            }
        }
    }

    fn emit(&mut self) {
        let time = Utc::now();

        macro_rules! emit_orig {
            ($map:ident, $metric_type:expr) => {
                for (name, mut inner_map) in self.$map.drain() {
                    for (tag_map, value) in inner_map.drain() {
                        let record = MetricRecord {
                            r#type: $metric_type,
                            name: name.clone(),
                            tag_map,
                            value,
                        };

                        for exporter in &self.exporters {
                            exporter.add_metric(time, &record);
                        }

                        if let Some(next) = &self.next {
                            next.add_metric(time, record, None);
                        }
                    }
                }
            };
        }

        macro_rules! emit_join {
            ($map:ident, $metric_type:expr) => {
                let mut joined_map: AHashMap<
                    Arc<MetricName>,
                    AHashMap<Arc<MetricTagMap>, MetricValue>,
                > = AHashMap::default();

                for (name, mut inner_map) in self.$map.drain() {
                    let joined_inner_map = joined_map.entry(name.clone()).or_default();

                    for (mut tag_map, value) in inner_map.drain() {
                        let inner = Arc::make_mut(&mut tag_map);
                        for tag in &self.config.join_tags {
                            inner.drop(tag);
                        }
                        joined_inner_map
                            .entry(tag_map)
                            .and_modify(|v| *v += value)
                            .or_insert(value);
                    }
                }

                for (name, inner_map) in joined_map.drain() {
                    for (tag_map, value) in inner_map {
                        let record = MetricRecord {
                            r#type: $metric_type,
                            name: name.clone(),
                            tag_map,
                            value,
                        };

                        for exporter in &self.exporters {
                            exporter.add_metric(time, &record);
                        }

                        if let Some(next) = &self.next {
                            next.add_metric(time, record, None);
                        }
                    }
                }
            };
        }

        if self.config.join_tags.is_empty() {
            emit_orig!(counter, MetricType::Counter);
            emit_orig!(gauge, MetricType::Gauge);
        } else {
            emit_join!(counter, MetricType::Counter);
            emit_join!(gauge, MetricType::Gauge);
        }
    }
}
