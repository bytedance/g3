/*
 * SPDX-License-Identifier: Apache-2.0
 * Copyright 2023-2025 ByteDance and/or its affiliates.
 */

use std::hash::Hash;
use std::sync::Arc;
use std::time::Duration;

use tokio::sync::{mpsc, oneshot};
use tokio::time::Instant;

mod runtime;
pub use runtime::EffectiveCacheRuntime;

mod handle;
pub use handle::{EffectiveCacheHandle, EffectiveQueryHandle};

pub struct EffectiveCacheData<R> {
    value: Option<R>,
    expire_at: Instant,
    vanish_at: Instant,
}

impl<R> EffectiveCacheData<R> {
    pub fn inner(&self) -> Option<&R> {
        self.value.as_ref()
    }

    pub fn new(data: R, ttl: u32, vanish_wait: Duration) -> Self {
        let now = Instant::now();
        let expire_at = now
            .checked_add(Duration::from_secs(ttl as u64))
            .unwrap_or(now);
        let vanish_at = expire_at.checked_add(vanish_wait).unwrap_or(expire_at);

        EffectiveCacheData {
            value: Some(data),
            expire_at,
            vanish_at,
        }
    }

    pub fn empty(protective_ttl: u32, vanish_wait: Duration) -> Self {
        let now = Instant::now();
        let expire_at = now
            .checked_add(Duration::from_secs(protective_ttl as u64))
            .unwrap_or(now);
        let vanish_at = expire_at.checked_add(vanish_wait).unwrap_or(expire_at);
        EffectiveCacheData {
            value: None,
            expire_at,
            vanish_at,
        }
    }
}

pub struct CacheQueryRequest<K, R> {
    cache_key: Arc<K>,
    query_cache_only: bool,
    notifier: oneshot::Sender<Arc<EffectiveCacheData<R>>>,
}

pub fn create_effective_cache<K: Hash + Eq, R: Send + Sync>(
    request_batch_handle_count: usize,
) -> (
    EffectiveCacheRuntime<K, R>,
    EffectiveCacheHandle<K, R>,
    EffectiveQueryHandle<K, R>,
) {
    let (rsp_sender, rsp_receiver) = mpsc::unbounded_channel();
    let (query_sender, query_receiver) = mpsc::unbounded_channel();
    let (req_sender, req_receiver) = mpsc::unbounded_channel();
    let cache_runtime = EffectiveCacheRuntime::new(
        request_batch_handle_count,
        req_receiver,
        rsp_receiver,
        query_sender,
    );
    let cache_handle = EffectiveCacheHandle::new(req_sender);
    let query_handle = EffectiveQueryHandle::new(query_receiver, rsp_sender);
    (cache_runtime, cache_handle, query_handle)
}
