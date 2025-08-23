/*
 * SPDX-License-Identifier: Apache-2.0
 * Copyright 2025 ByteDance and/or its affiliates.
 */

use std::sync::Arc;

use tokio::sync::broadcast;
use tokio::time::Instant;

use crate::config::collector::internal::InternalCollectorConfig;

pub(super) struct InternalEmitter {
    reload_receiver: broadcast::Receiver<Arc<InternalCollectorConfig>>,
}

impl InternalEmitter {
    pub(super) fn new(reload_receiver: broadcast::Receiver<Arc<InternalCollectorConfig>>) -> Self {
        InternalEmitter { reload_receiver }
    }

    pub(super) async fn into_running(mut self, mut config: Arc<InternalCollectorConfig>) {
        let mut interval = tokio::time::interval(config.emit_interval);

        let mut last_instant = Instant::now();
        loop {
            tokio::select! {
                i = interval.tick() => {
                    last_instant = i;
                    // TODO emit stats
                }
                r = self.reload_receiver.recv() => {
                    match r {
                        Ok(c) => {
                            let next_tick = last_instant + interval.period();
                            config = c;
                            interval = tokio::time::interval_at(next_tick, config.emit_interval);
                        }
                        Err(broadcast::error::RecvError::Closed) => {
                            break;
                        }
                        Err(broadcast::error::RecvError::Lagged(_)) => {}
                    }
                }
            }
        }
    }
}
