/*
 * SPDX-License-Identifier: Apache-2.0
 * Copyright 2025 ByteDance and/or its affiliates.
 */

use std::sync::Arc;

use tokio::sync::{Semaphore, broadcast};
use tokio::time::Instant;

use super::{AggregateHandle, Command};
use crate::config::collector::aggregate::AggregateCollectorConfig;

pub(super) struct EmitTimer {
    config: Arc<AggregateCollectorConfig>,
    handle: Arc<AggregateHandle>,
    cfg_receiver: broadcast::Receiver<Arc<AggregateCollectorConfig>>,
}

impl EmitTimer {
    pub(super) fn new(
        config: Arc<AggregateCollectorConfig>,
        handle: Arc<AggregateHandle>,
        cfg_receiver: broadcast::Receiver<Arc<AggregateCollectorConfig>>,
    ) -> Self {
        EmitTimer {
            config,
            handle,
            cfg_receiver,
        }
    }

    pub(super) async fn into_running(mut self) {
        let mut interval = tokio::time::interval(self.config.emit_interval);
        let mut last_tick = Instant::now();

        loop {
            tokio::select! {
                r = self.cfg_receiver.recv() => {
                    match r {
                        Ok(config) => {
                            self.config = config;
                            interval = tokio::time::interval_at(last_tick + interval.period(), self.config.emit_interval);
                        }
                        Err(broadcast::error::RecvError::Closed) => break,
                        Err(broadcast::error::RecvError::Lagged(_)) => continue,
                    }
                }
                instant = interval.tick() => {
                    last_tick = instant;
                    self.notify_emit().await;
                }
            }
        }
    }

    async fn notify_emit(&mut self) {
        let semaphore = Arc::new(Semaphore::new(0));
        for worker in &self.handle.worker {
            let _ = worker.send(Command::Sync(semaphore.clone()));
        }
        let _ = semaphore
            .acquire_many(self.handle.worker.len() as u32)
            .await;
        let _ = self.handle.global.send(Command::Emit);
    }
}
