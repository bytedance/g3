/*
 * SPDX-License-Identifier: Apache-2.0
 * Copyright 2024-2025 ByteDance and/or its affiliates.
 */

use std::future::poll_fn;
use std::sync::Arc;
use std::time::Duration;

use chrono::Utc;
use log::warn;
use rustc_hash::FxHashSet;
use tokio::time::Instant;
use tokio_util::time::DelayQueue;

use g3_types::net::{OpensslTicketKey, RollingTicketKey, RollingTicketer, TicketKeyName};

use super::TlsTicketConfig;
use crate::source::TicketSource;

pub(crate) struct TicketKeyUpdate {
    config: TlsTicketConfig,
    ticketer: Arc<RollingTicketer<OpensslTicketKey>>,
    expire_set: FxHashSet<TicketKeyName>,
    expire_queue: DelayQueue<TicketKeyName>,
    local_roll_at: Instant,
}

impl TicketKeyUpdate {
    pub(crate) fn new(
        config: TlsTicketConfig,
        ticketer: Arc<RollingTicketer<OpensslTicketKey>>,
    ) -> Self {
        let local_roll_time = Duration::from_secs((config.local_lifetime >> 1) as u64);
        let local_roll_at = Instant::now() + local_roll_time;
        TicketKeyUpdate {
            config,
            ticketer,
            expire_set: FxHashSet::default(),
            expire_queue: DelayQueue::new(),
            local_roll_at,
        }
    }

    pub(crate) fn spawn_run(self) {
        tokio::spawn(self.run());
    }

    async fn run(mut self) {
        let mut check_interval = tokio::time::interval(self.config.check_interval);

        let remote_source = match &self.config.remote_source {
            Some(config) => match config.build() {
                Ok(source) => Some(source),
                Err(e) => {
                    warn!("remote source disabled, dur to: {e}");
                    None
                }
            },
            None => None,
        };

        loop {
            if self.expire_set.is_empty() {
                check_interval.tick().await;
                self.check_roll_ticket(remote_source.as_ref()).await;
            } else {
                tokio::select! {
                    biased;

                    _ = check_interval.tick() => {
                        self.check_roll_ticket(remote_source.as_ref()).await;
                    }
                    v = poll_fn(|cx| self.expire_queue.poll_expired(cx)) => {
                        if let Some(expired) = v {
                            let name = expired.into_inner();
                            self.expire_set.remove(&name);
                            self.ticketer.del_decrypt_key(name);
                        }
                    }
                }
            }

            if Arc::strong_count(&self.ticketer) == 1 {
                break;
            }
        }
    }

    async fn check_roll_ticket(&mut self, remote_source: Option<&TicketSource>) {
        let mut roll_local = true;
        if let Some(source) = &remote_source {
            match source.fetch_remote_keys().await {
                Ok(data) => {
                    roll_local = false;
                    self.update_encrypt_key(data.enc.key, Instant::now());
                    let now = Utc::now();
                    for dec_key in data.dec {
                        if let Some(expire_dur) = dec_key.expire_duration(&now) {
                            let key = dec_key.key;
                            let key_name = key.name();
                            if !self.expire_set.contains(&key_name) {
                                self.ticketer.add_decrypt_key(Arc::new(key));
                                self.expire_set.insert(key_name);
                                self.expire_queue.insert(key_name, expire_dur);
                            }
                        }
                    }
                }
                Err(e) => {
                    warn!("failed to get keys from remote source: {e}")
                }
            }
        }

        let now = Instant::now();
        if roll_local && self.local_roll_at <= now {
            self.new_local_key(now);
        }
    }

    fn new_local_key(&mut self, now: Instant) {
        let local_lifetime = self.config.local_lifetime;
        match OpensslTicketKey::new_random(local_lifetime) {
            Ok(key) => self.update_encrypt_key(key, now),
            Err(e) => warn!("failed to create new ticket key: {e}"),
        }
    }

    fn update_encrypt_key(&mut self, key: OpensslTicketKey, now: Instant) {
        let old_key = self.ticketer.encrypt_key();
        let old_key_name = old_key.name();
        if !self.expire_set.contains(&old_key_name) {
            // maybe a local generated key, or a remote enc key but not in dec list
            let expire_time = Duration::from_secs(old_key.lifetime() as u64);
            self.expire_set.insert(old_key_name);
            self.expire_queue.insert(old_key_name, expire_time);
        }
        let local_roll_time = Duration::from_secs((key.lifetime() >> 1) as u64);
        self.local_roll_at = now + local_roll_time;
        let key = Arc::new(key);
        self.ticketer.set_encrypt_key(key.clone());
        self.ticketer.add_decrypt_key(key);
    }
}
