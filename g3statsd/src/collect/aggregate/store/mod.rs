/*
 * SPDX-License-Identifier: Apache-2.0
 * Copyright 2025 ByteDance and/or its affiliates.
 */

use std::sync::Arc;

use tokio::sync::{Semaphore, broadcast, mpsc};

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
    Sync(Arc<Semaphore>),
    Emit,
}

pub(super) struct AggregateHandle {
    worker: Vec<mpsc::UnboundedSender<Command>>,
    global: mpsc::UnboundedSender<Command>,
}

impl AggregateHandle {
    pub(super) fn spawn_new(
        config: Arc<AggregateCollectorConfig>,
        cfg_receiver: broadcast::Receiver<Arc<AggregateCollectorConfig>>,
    ) -> Arc<Self> {
        let (global_cmd_sender, global_cmd_receiver) = mpsc::unbounded_channel();

        let global_store = GlobalStore::new(
            config.clone(),
            cfg_receiver.resubscribe(),
            global_cmd_receiver,
        );
        tokio::spawn(global_store.into_running());

        let mut worker_senders = Vec::new();
        let _: Result<usize, ()> = g3_daemon::runtime::worker::foreach(|handle| {
            let (worker_sender, worker_receiver) = mpsc::unbounded_channel();

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
        match record.r#type {
            MetricType::Counter => {
                if let Some(id) = worker_id
                    && let Some(sender) = self.worker.get(id)
                {
                    if sender.send(Command::Add(record)).is_err() {
                        // TODO add stats
                    }
                    return;
                }
            }
            MetricType::Gauge => {}
        }

        if self.global.send(Command::Add(record)).is_err() {
            // TODO add stats
        }
    }
}
