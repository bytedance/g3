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

use g3_types::metrics::MetricTagMap;

use super::Command;
use crate::types::{MetricName, MetricRecord, MetricType, MetricValue};

const BATCH_SIZE: usize = 128;

pub(super) struct WorkerStore {
    receiver: mpsc::UnboundedReceiver<Command>,
    global_sender: mpsc::UnboundedSender<Command>,

    counter: AHashMap<Arc<MetricName>, AHashMap<Arc<MetricTagMap>, MetricValue>>,
}

impl WorkerStore {
    pub(super) fn new(
        receiver: mpsc::UnboundedReceiver<Command>,
        global_sender: mpsc::UnboundedSender<Command>,
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
                break;
            }

            while let Some(cmd) = buffer.pop() {
                match cmd {
                    Command::Add(record) => self.add_record(record),
                    Command::Sync(semaphore) => {
                        self.emit();
                        semaphore.add_permits(1);
                    }
                    Command::Emit => unreachable!(),
                }
            }
        }

        self.emit();
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
                let _ = self.global_sender.send(Command::Add(record));
            }
        }
    }

    fn emit(&mut self) {
        for (name, mut inner_map) in self.counter.drain() {
            for (tag_map, value) in inner_map.drain() {
                let record = MetricRecord {
                    r#type: MetricType::Counter,
                    name: name.clone(),
                    tag_map,
                    value,
                };
                let _ = self.global_sender.send(Command::Add(record));
            }
        }
    }
}
