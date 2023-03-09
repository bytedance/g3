/*
 * Copyright 2023 ByteDance and/or its affiliates.
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

use std::collections::hash_map;
use std::hash::Hash;
use std::sync::Arc;
use std::task::{Context, Poll};
use std::time::Duration;

use ahash::AHashMap;
use tokio::sync::{mpsc, oneshot};
use tokio_util::time::{delay_queue, DelayQueue};

use super::{CacheQueryRequest, EffectiveCacheData};

#[derive(Clone)]
pub struct EffectiveCacheHandle<K, R> {
    req_sender: mpsc::UnboundedSender<CacheQueryRequest<K, R>>,
}

impl<K, R> EffectiveCacheHandle<K, R> {
    pub(super) fn new(req_sender: mpsc::UnboundedSender<CacheQueryRequest<K, R>>) -> Self {
        EffectiveCacheHandle { req_sender }
    }

    pub async fn fetch(
        &self,
        cache_key: Arc<K>,
        timeout: Duration,
    ) -> Option<Arc<EffectiveCacheData<R>>> {
        let (rsp_sender, rsp_receiver) = oneshot::channel();
        let req = CacheQueryRequest {
            cache_key,
            notifier: rsp_sender,
        };
        self.req_sender.send(req).ok()?;

        match tokio::time::timeout(timeout, rsp_receiver).await {
            Ok(Ok(r)) => Some(r),
            Ok(Err(_)) => None, // recv error
            Err(_) => None,     // timeout
        }
    }
}

pub struct EffectiveQueryHandle<K: Hash + Eq, R> {
    req_receiver: mpsc::UnboundedReceiver<Arc<K>>,
    rsp_sender: mpsc::UnboundedSender<(Arc<K>, EffectiveCacheData<R>)>,
    doing_cache: AHashMap<Arc<K>, delay_queue::Key>,
    doing_timeout_queue: DelayQueue<Arc<K>>,
}

impl<K: Hash + Eq, R> EffectiveQueryHandle<K, R> {
    pub(super) fn new(
        req_receiver: mpsc::UnboundedReceiver<Arc<K>>,
        rsp_sender: mpsc::UnboundedSender<(Arc<K>, EffectiveCacheData<R>)>,
    ) -> Self {
        EffectiveQueryHandle {
            req_receiver,
            rsp_sender,
            doing_cache: AHashMap::new(),
            doing_timeout_queue: DelayQueue::new(),
        }
    }

    pub fn poll_recv_req(&mut self, cx: &mut Context<'_>) -> Poll<Option<Arc<K>>> {
        self.req_receiver.poll_recv(cx)
    }

    pub fn poll_query_expired(&mut self, cx: &mut Context<'_>) -> Poll<Option<Arc<K>>> {
        match self.doing_timeout_queue.poll_expired(cx) {
            Poll::Pending => Poll::Pending,
            Poll::Ready(None) => Poll::Ready(None),
            Poll::Ready(Some(e)) => Poll::Ready(Some(e.into_inner())),
        }
    }

    pub fn send_rsp_data(&mut self, req: Arc<K>, data: EffectiveCacheData<R>, expired: bool) {
        if let Some(timeout_key) = self.doing_cache.remove(&req) {
            if !expired {
                self.doing_timeout_queue.remove(&timeout_key);
            }
            let _ = self.rsp_sender.send((req, data));
        }
    }

    pub fn should_send_raw_query(&mut self, req: Arc<K>, query_wait: Duration) -> bool {
        match self.doing_cache.entry(req) {
            hash_map::Entry::Occupied(_) => false,
            hash_map::Entry::Vacant(cv) => {
                let timeout_key = self
                    .doing_timeout_queue
                    .insert(Arc::clone(cv.key()), query_wait);
                cv.insert(timeout_key);
                true
            }
        }
    }
}
