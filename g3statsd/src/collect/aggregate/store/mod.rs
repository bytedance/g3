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

use tokio::sync::{broadcast, mpsc, oneshot};

use crate::config::collector::aggregate::AggregateCollectorConfig;
use crate::types::{MetricRecord, MetricType};

mod timer;
use timer::EmitTimer;

mod global;
use global::GlobalStore;

mod worker;
use worker::WorkerStore;

enum Command {
    Add(MetricRecord),
    Emit(oneshot::Sender<usize>),
}

pub(super) struct AggregateHandle {
    worker: Vec<mpsc::Sender<Command>>,
    global: mpsc::Sender<Command>,
}

impl AggregateHandle {
    pub(super) fn spawn_new(
        config: Arc<AggregateCollectorConfig>,
        cfg_receiver: broadcast::Receiver<Arc<AggregateCollectorConfig>>,
    ) -> Arc<Self> {
        let (global_cmd_sender, global_cmd_receiver) = mpsc::channel(512);

        let global_store = GlobalStore::new(
            config.clone(),
            cfg_receiver.resubscribe(),
            global_cmd_receiver,
        );
        tokio::spawn(global_store.into_running());

        let mut worker_senders = Vec::new();
        let _: Result<usize, ()> = g3_daemon::runtime::worker::foreach(|handle| {
            let (worker_sender, worker_receiver) = mpsc::channel(128);

            let worker_store = WorkerStore::new(worker_receiver, global_cmd_sender.clone());
            handle.handle.spawn(worker_store.into_running());
            worker_senders.push(worker_sender);
            Ok(())
        });

        let handle = Arc::new(AggregateHandle {
            worker: worker_senders,
            global: global_cmd_sender,
        });

        let emit_timer = EmitTimer::new(config, handle.clone(), cfg_receiver);
        tokio::spawn(emit_timer.into_running());

        handle
    }

    pub(super) fn add_metric(&self, record: MetricRecord, worker_id: Option<usize>) {
        use mpsc::error::TrySendError;

        match record.r#type {
            MetricType::Counter => {
                if let Some(id) = worker_id {
                    if let Some(sender) = self.worker.get(id) {
                        match sender.try_send(Command::Add(record)) {
                            Ok(_) => {}
                            Err(TrySendError::Full(msg)) => {
                                let sender = sender.clone();
                                tokio::spawn(async move {
                                    let _ = sender.send(msg).await; // TODO add stats
                                });
                            }
                            Err(TrySendError::Closed(_msg)) => {
                                // TODO add stats
                            }
                        }
                        return;
                    }
                }
            }
            MetricType::Gauge => {}
        }

        match self.global.try_send(Command::Add(record)) {
            Ok(_) => {}
            Err(TrySendError::Full(msg)) => {
                let sender = self.global.clone();
                tokio::spawn(async move {
                    let _ = sender.send(msg).await; // TODO add stats
                });
            }
            Err(TrySendError::Closed(_msg)) => {
                // TODO add stats
            }
        }
    }
}
