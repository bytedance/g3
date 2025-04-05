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
use tokio::sync::mpsc;

use super::{Command, StoreRecord};
use crate::types::{MetricName, MetricTagMap, MetricType, MetricValue};

const BATCH_SIZE: usize = 128;

pub(super) struct WorkerStore {
    receiver: mpsc::Receiver<Command>,
    global_sender: mpsc::Sender<Command>,

    counter: AHashMap<Arc<MetricName>, AHashMap<Arc<MetricTagMap>, MetricValue>>,
}

impl WorkerStore {
    pub(super) fn new(
        receiver: mpsc::Receiver<Command>,
        global_sender: mpsc::Sender<Command>,
    ) -> Self {
        WorkerStore {
            receiver,
            global_sender,
            counter: Default::default(),
        }
    }

    pub(super) async fn into_running(mut self) {
        let mut buffer = Vec::with_capacity(BATCH_SIZE);
        loop {
            let nr = self.receiver.recv_many(&mut buffer, BATCH_SIZE).await;
            if nr == 0 {
                let _ = self.emit().await;
                break;
            }

            while let Some(cmd) = buffer.pop() {
                match cmd {
                    Command::Add(record) => self.add_record(record).await,
                    Command::Emit(sender) => match self.emit().await {
                        Ok(n) => {
                            let _ = sender.send(n);
                        }
                        Err(n) => {
                            let _ = sender.send(n);
                        }
                    },
                }
            }
        }
    }

    async fn add_record(&mut self, record: StoreRecord) {
        match record.r#type {
            MetricType::Counter => {
                let StoreRecord {
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
                let _ = self.global_sender.send(Command::Add(record)).await;
            }
        }
    }

    async fn emit(&mut self) -> Result<usize, usize> {
        let mut emit_total = 0;

        for (name, mut inner_map) in self.counter.drain() {
            for (tag_map, value) in inner_map.drain() {
                let record = StoreRecord {
                    r#type: MetricType::Counter,
                    name: name.clone(),
                    tag_map: tag_map.clone(),
                    value,
                };
                match self.global_sender.send(Command::Add(record)).await {
                    Ok(_) => {
                        emit_total += 1;
                    }
                    Err(_) => {
                        return Err(emit_total);
                    }
                }
            }
        }

        Ok(emit_total)
    }
}
