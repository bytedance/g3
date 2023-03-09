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
use std::future::Future;
use std::hash::Hash;
use std::io;
use std::pin::Pin;
use std::sync::Arc;
use std::task::{Context, Poll};

use ahash::AHashMap;
use tokio::sync::mpsc;
use tokio::time::Instant;
use tokio_util::time::{delay_queue, DelayQueue};

use super::{CacheQueryRequest, EffectiveCacheData};

struct CacheQueryValue<R> {
    result: Arc<EffectiveCacheData<R>>,
    vanish_key: Option<delay_queue::Key>,
}

pub struct EffectiveCacheRuntime<K: Hash, R> {
    request_batch_handle_count: usize,
    cache: AHashMap<Arc<K>, CacheQueryValue<R>>,
    doing: AHashMap<Arc<K>, Vec<Option<CacheQueryRequest<K, R>>>>,
    req_receiver: mpsc::UnboundedReceiver<CacheQueryRequest<K, R>>,
    rsp_receiver: mpsc::UnboundedReceiver<(Arc<K>, EffectiveCacheData<R>)>,
    query_sender: mpsc::UnboundedSender<Arc<K>>,
    vanish: DelayQueue<Arc<K>>,
}

impl<K: Hash + Eq, R> EffectiveCacheRuntime<K, R> {
    pub(super) fn new(
        request_batch_handle_count: usize,
        req_receiver: mpsc::UnboundedReceiver<CacheQueryRequest<K, R>>,
        rsp_receiver: mpsc::UnboundedReceiver<(Arc<K>, EffectiveCacheData<R>)>,
        query_sender: mpsc::UnboundedSender<Arc<K>>,
    ) -> Self {
        EffectiveCacheRuntime {
            request_batch_handle_count,
            cache: AHashMap::new(),
            doing: AHashMap::new(),
            req_receiver,
            rsp_receiver,
            query_sender,
            vanish: DelayQueue::new(),
        }
    }

    fn handle_rsp(&mut self, key: Arc<K>, result: Arc<EffectiveCacheData<R>>) {
        if let Some(vec) = self.doing.remove(&key) {
            for req in vec.into_iter().flatten() {
                let _ = req.notifier.send(Arc::clone(&result));
            }

            match self.cache.entry(Arc::clone(&key)) {
                hash_map::Entry::Occupied(mut o) => {
                    let ov = o.get_mut();
                    let vanish_key = if let Some(vanish_key) = ov.vanish_key.take() {
                        self.vanish.reset_at(&vanish_key, result.vanish_at);
                        vanish_key
                    } else {
                        self.vanish.insert_at(key, result.vanish_at)
                    };
                    ov.vanish_key = Some(vanish_key);
                    ov.result = result;
                }
                hash_map::Entry::Vacant(v) => {
                    let vanish_key = self.vanish.insert_at(key, result.vanish_at);
                    v.insert(CacheQueryValue {
                        result,
                        vanish_key: Some(vanish_key),
                    });
                }
            }
        } else {
            // ignore those has been answered
        }
    }

    fn handle_vanish(&mut self, key: Arc<K>) {
        self.cache.remove(&key);
    }

    fn send_req(&mut self, key: Arc<K>) {
        if self.query_sender.send(key).is_err() {
            // the query runtime should not close before the cache runtime
            unreachable!()
        }
    }

    fn handle_req(&mut self, req: CacheQueryRequest<K, R>) {
        if let Some(v) = self.cache.get(&req.cache_key) {
            let _ = req.notifier.send(Arc::clone(&v.result));
            if v.result.expire_at < Instant::now() {
                // update if expired
                match self.doing.entry(Arc::clone(&req.cache_key)) {
                    hash_map::Entry::Occupied(_) => {}
                    hash_map::Entry::Vacant(v) => {
                        v.insert(vec![None]);
                        self.send_req(Arc::clone(&req.cache_key));
                    }
                }
            }
        } else {
            match self.doing.entry(Arc::clone(&req.cache_key)) {
                hash_map::Entry::Occupied(mut o) => {
                    o.get_mut().push(Some(req));
                }
                hash_map::Entry::Vacant(v) => {
                    let req_key = Arc::clone(&req.cache_key);
                    v.insert(vec![Some(req)]);
                    self.send_req(req_key);
                }
            };
        }
    }

    fn poll_loop(&mut self, cx: &mut Context<'_>) -> Poll<io::Result<()>> {
        loop {
            // handle rsp
            loop {
                match self.rsp_receiver.poll_recv(cx) {
                    Poll::Pending => break,
                    Poll::Ready(None) => unreachable!(), // unreachable as we have kept a sender
                    Poll::Ready(Some((k, r))) => self.handle_rsp(k, Arc::new(r)),
                }
            }

            // handle vanish
            loop {
                match self.vanish.poll_expired(cx) {
                    Poll::Pending => break,
                    Poll::Ready(None) => break, // all items fetched
                    Poll::Ready(Some(t)) => self.handle_vanish(t.into_inner()),
                }
            }

            // handle req
            for _ in 1..self.request_batch_handle_count {
                match self.req_receiver.poll_recv(cx) {
                    Poll::Pending => return Poll::Pending,
                    Poll::Ready(None) => return Poll::Ready(Ok(())),
                    Poll::Ready(Some(req)) => self.handle_req(req),
                }
            }
        }
    }
}

impl<K: Hash + Eq, R> Future for EffectiveCacheRuntime<K, R> {
    type Output = io::Result<()>;

    fn poll(mut self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Self::Output> {
        (*self).poll_loop(cx)
    }
}
