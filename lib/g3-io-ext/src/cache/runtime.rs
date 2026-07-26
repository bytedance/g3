/*
 * SPDX-License-Identifier: Apache-2.0
 * SPDX-FileCopyrightText: 2023-2025 ByteDance and/or its affiliates.
 * SPDX-FileCopyrightText: 2026 VEY-OSS Developers.
 */

use std::collections::hash_map;
use std::hash::Hash;
use std::io;
use std::num::NonZeroUsize;
use std::pin::Pin;
use std::sync::Arc;
use std::task::{Context, Poll};

use ahash::AHashMap;
use tokio::sync::mpsc;
use tokio::time::Instant;
use tokio_util::time::{DelayQueue, delay_queue};

use super::{CacheQueryRequest, EffectiveCacheData};

struct CacheQueryValue<R> {
    result: Arc<EffectiveCacheData<R>>,
    vanish_key: Option<delay_queue::Key>,
}

pub struct EffectiveCacheRuntime<K: Hash, R> {
    request_batch_handle_count: NonZeroUsize,
    cache: AHashMap<Arc<K>, CacheQueryValue<R>>,
    doing: AHashMap<Arc<K>, Vec<CacheQueryRequest<K, R>>>,
    req_receiver: mpsc::UnboundedReceiver<CacheQueryRequest<K, R>>,
    rsp_receiver: mpsc::UnboundedReceiver<(Arc<K>, EffectiveCacheData<R>)>,
    query_sender: mpsc::UnboundedSender<Arc<K>>,
    vanish: DelayQueue<Arc<K>>,
}

impl<K: Hash + Eq, R: Send + Sync> EffectiveCacheRuntime<K, R> {
    pub(super) fn new(
        request_batch_handle_count: NonZeroUsize,
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
            for req in vec {
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
            // ignore those have been answered
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
        if req.query_cache_only {
            if let Some(v) = self.cache.get(&req.cache_key) {
                let _ = req.notifier.send(v.result.clone());
            }
            return;
        }

        if let Some(v) = self.cache.get(&req.cache_key) {
            let _ = req.notifier.send(Arc::clone(&v.result));
            if v.result.expire_at < Instant::now() {
                // update if expired
                match self.doing.entry(Arc::clone(&req.cache_key)) {
                    hash_map::Entry::Occupied(_) => {}
                    hash_map::Entry::Vacant(v) => {
                        v.insert(vec![]);
                        self.send_req(Arc::clone(&req.cache_key));
                    }
                }
            }
        } else {
            match self.doing.entry(Arc::clone(&req.cache_key)) {
                hash_map::Entry::Occupied(mut o) => {
                    o.get_mut().push(req);
                }
                hash_map::Entry::Vacant(v) => {
                    let req_key = Arc::clone(&req.cache_key);
                    v.insert(vec![req]);
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
            for _ in 0..self.request_batch_handle_count.get() {
                match self.req_receiver.poll_recv(cx) {
                    Poll::Pending => return Poll::Pending,
                    Poll::Ready(None) => return Poll::Ready(Ok(())),
                    Poll::Ready(Some(req)) => self.handle_req(req),
                }
            }
        }
    }
}

impl<K: Hash + Eq, R: Send + Sync> Future for EffectiveCacheRuntime<K, R> {
    type Output = io::Result<()>;

    fn poll(mut self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Self::Output> {
        (*self).poll_loop(cx)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::cache::create_effective_cache;
    use std::time::Duration;

    #[tokio::test]
    async fn test_request_batch_handle_count_one() {
        let (runtime, handle, mut query_handle) =
            create_effective_cache::<String, String>(NonZeroUsize::MIN);

        let runtime_task = tokio::spawn(runtime);

        let key = Arc::new("key1".to_string());
        let fetch_handle = handle.clone();
        let key_clone = key.clone();
        let fetch_task = tokio::spawn(async move {
            fetch_handle.fetch(key_clone, Duration::from_secs(5)).await
        });

        // Query handle receives the request key via poll_recv_req
        let req_key = std::future::poll_fn(|cx| query_handle.poll_recv_req(cx))
            .await
            .expect("should receive request key");
        assert_eq!(*req_key, "key1");

        assert!(query_handle.should_send_raw_query(req_key.clone(), Duration::from_secs(10)));
        query_handle.send_rsp_data(
            req_key,
            EffectiveCacheData::new("val1".to_string(), 60, Duration::from_secs(60)),
            false,
        );

        let res = fetch_task.await.unwrap().expect("fetch should succeed");
        assert_eq!(res.inner().map(|s| s.as_str()), Some("val1"));

        drop(handle);
        drop(query_handle);
        let _ = runtime_task.await;
    }
}
