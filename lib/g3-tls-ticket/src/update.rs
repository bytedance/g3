/*
 * Copyright 2024 ByteDance and/or its affiliates.
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

use std::future::poll_fn;
use std::sync::Arc;
use std::time::Duration;

use log::warn;
use tokio::time::Instant;
use tokio_util::time::DelayQueue;

use g3_types::net::{OpensslTicketKey, RollingTicketKey, RollingTicketer, TicketKeyName};

use super::TlsTicketConfig;

pub(crate) struct TicketKeyUpdate {
    config: TlsTicketConfig,
    ticketer: Arc<RollingTicketer<OpensslTicketKey>>,
    expire_queue: DelayQueue<TicketKeyName>,
}

impl TicketKeyUpdate {
    pub(crate) fn new(
        config: TlsTicketConfig,
        ticketer: Arc<RollingTicketer<OpensslTicketKey>>,
    ) -> Self {
        TicketKeyUpdate {
            config,
            ticketer,
            expire_queue: DelayQueue::new(),
        }
    }

    pub(crate) fn spawn_run(self) {
        tokio::spawn(self.run());
    }

    async fn run(mut self) {
        let mut check_interval = tokio::time::interval(self.config.check_interval);
        let local_lifetime = self.config.local_lifetime;
        let expire_time = Duration::from_secs((local_lifetime >> 1) as u64);
        let mut expire_at = Instant::now() + expire_time;

        loop {
            tokio::select! {
                biased;

                _ = check_interval.tick() => {
                    // TODO fetch from remote source

                    let now = Instant::now();
                    if expire_at >= now {
                        match OpensslTicketKey::new_random(local_lifetime) {
                            Ok(key) => {
                                self.ticketer.set_encrypt_key(Arc::new(key));
                                expire_at = now + expire_time;
                            }
                            Err(e) => warn!("failed to create new ticket key: {e}"),
                        }
                    }
                }
                v = poll_fn(|cx| self.expire_queue.poll_expired(cx)) => {
                    if let Some(expired) = v {
                        self.ticketer.del_decrypt_key(expired.into_inner());
                    }
                }
            }
        }
    }
}
