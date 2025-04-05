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

use ahash::AHashMap;
use tokio::sync::{mpsc, oneshot};

use crate::types::{MetricName, MetricRecord, MetricTagMap, MetricType, MetricValue};

mod global;
mod worker;

enum Command {
    Add(MetricRecord),
    Emit(oneshot::Sender<usize>),
}

pub(super) struct AggregateHandle {
    worker: Vec<mpsc::Sender<Command>>,
    global: mpsc::Sender<Command>,
}

#[derive(Default)]
struct AggregateStore {
    counter: AHashMap<MetricName, AHashMap<MetricTagMap, MetricValue>>,
    gauge: AHashMap<MetricName, AHashMap<MetricTagMap, MetricValue>>,
}

impl AggregateStore {
    fn add(&mut self, record: MetricRecord) {
        let top_map = match record.r#type {
            MetricType::Counter => &mut self.counter,
            MetricType::Gauge => &mut self.gauge,
        };

        let MetricRecord {
            r#type: _,
            name,
            tag_map,
            value,
        } = record;

        top_map
            .entry(name)
            .or_default()
            .entry(tag_map)
            .and_modify(|v| *v += value)
            .or_insert(value);
    }

    async fn emit(&mut self, sender: &mpsc::Sender<Command>) -> Result<usize, usize> {
        let mut emit_total = 0;

        macro_rules! emit {
            ($map:ident, $metric_type:expr) => {
                for (name, mut inner_map) in self.$map.drain() {
                    for (tag_map, value) in inner_map.drain() {
                        let record = MetricRecord {
                            r#type: $metric_type,
                            name: name.clone(),
                            tag_map: tag_map.clone(),
                            value,
                        };
                        match sender.send(Command::Add(record)).await {
                            Ok(_) => {
                                emit_total += 1;
                            }
                            Err(_) => {
                                return Err(emit_total);
                            }
                        }
                    }
                }
            };
        }

        emit!(counter, MetricType::Counter);
        emit!(gauge, MetricType::Gauge);

        Ok(emit_total)
    }
}
