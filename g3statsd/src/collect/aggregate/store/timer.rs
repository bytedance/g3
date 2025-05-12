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

use log::{trace, warn};
use tokio::sync::{broadcast, oneshot};
use tokio::task::JoinSet;
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
        let mut join_set = JoinSet::new();
        for (i, worker) in self.handle.worker.iter().enumerate() {
            let worker = worker.clone();
            join_set.spawn(async move {
                let (sender, receiver) = oneshot::channel();
                let r = if worker.send(Command::Emit(sender)).is_ok() {
                    receiver.await
                } else {
                    Ok(0)
                };
                (i, r)
            });
        }
        while let Some(r) = join_set.join_next().await {
            match r {
                Ok((i, r)) => match r {
                    Ok(n) => {
                        trace!("worker {i} emit {n} metrics");
                    }
                    Err(_) => {
                        warn!("worker {i} emit metrics failed with no response");
                    }
                },
                Err(e) => {
                    warn!("join worker emit notify task error: {e}");
                }
            }
        }

        let (sender, receiver) = oneshot::channel();
        if self.handle.global.send(Command::Emit(sender)).is_ok() {
            let _ = receiver.await;
        }
    }
}
