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

use tokio::sync::broadcast;
use tokio::time::Instant;

use crate::config::collect::internal::InternalCollectConfig;

pub(super) struct InternalEmitter {
    reload_receiver: broadcast::Receiver<Arc<InternalCollectConfig>>,
}

impl InternalEmitter {
    pub(super) fn new(reload_receiver: broadcast::Receiver<Arc<InternalCollectConfig>>) -> Self {
        InternalEmitter { reload_receiver }
    }

    pub(super) async fn into_running(mut self, mut config: Arc<InternalCollectConfig>) {
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
