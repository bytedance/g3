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

use std::collections::hash_map;
use std::net::IpAddr;
use std::sync::Arc;
use std::task::{Context, Poll};
use std::time::Duration;

use rustc_hash::FxHashMap;
use tokio::sync::{mpsc, oneshot};
use tokio_util::time::{DelayQueue, delay_queue};

use g3_geoip_types::IpLocation;

use super::{CacheQueryRequest, IpLocationCacheResponse};

pub struct IpLocationServiceHandle {
    cache_handle: IpLocationCacheHandle,
    request_timeout: Duration,
}

impl IpLocationServiceHandle {
    pub(crate) fn new(cache_handle: IpLocationCacheHandle, request_timeout: Duration) -> Self {
        IpLocationServiceHandle {
            cache_handle,
            request_timeout,
        }
    }

    pub async fn fetch(&self, ip: IpAddr) -> Option<Arc<IpLocation>> {
        self.cache_handle.fetch(ip, self.request_timeout).await
    }
}

pub(crate) struct IpLocationCacheHandle {
    req_sender: mpsc::UnboundedSender<CacheQueryRequest>,
}

impl IpLocationCacheHandle {
    pub(crate) fn new(req_sender: mpsc::UnboundedSender<CacheQueryRequest>) -> Self {
        IpLocationCacheHandle { req_sender }
    }

    async fn fetch(&self, ip: IpAddr, timeout: Duration) -> Option<Arc<IpLocation>> {
        let (rsp_sender, rsp_receiver) = oneshot::channel();
        let req = CacheQueryRequest {
            ip,
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

pub(super) struct IpLocationQueryHandle {
    req_receiver: mpsc::UnboundedReceiver<IpAddr>,
    rsp_sender: mpsc::UnboundedSender<(Option<IpAddr>, IpLocationCacheResponse)>,
    doing_cache: FxHashMap<IpAddr, delay_queue::Key>,
    doing_timeout_queue: DelayQueue<IpAddr>,
}

impl IpLocationQueryHandle {
    pub(super) fn new(
        req_receiver: mpsc::UnboundedReceiver<IpAddr>,
        rsp_sender: mpsc::UnboundedSender<(Option<IpAddr>, IpLocationCacheResponse)>,
    ) -> Self {
        IpLocationQueryHandle {
            req_receiver,
            rsp_sender,
            doing_cache: FxHashMap::default(),
            doing_timeout_queue: DelayQueue::new(),
        }
    }

    pub(super) fn poll_recv_req(&mut self, cx: &mut Context<'_>) -> Poll<Option<IpAddr>> {
        self.req_receiver.poll_recv(cx)
    }

    pub(super) fn poll_query_expired(&mut self, cx: &mut Context<'_>) -> Poll<Option<IpAddr>> {
        match self.doing_timeout_queue.poll_expired(cx) {
            Poll::Pending => Poll::Pending,
            Poll::Ready(None) => Poll::Ready(None),
            Poll::Ready(Some(e)) => Poll::Ready(Some(e.into_inner())),
        }
    }

    pub(super) fn send_rsp_data(
        &mut self,
        ip: Option<IpAddr>,
        data: IpLocationCacheResponse,
        expired: bool,
    ) {
        if let Some(ip) = ip {
            if let Some(timeout_key) = self.doing_cache.remove(&ip) {
                if !expired {
                    self.doing_timeout_queue.remove(&timeout_key);
                }
            }
        }
        let _ = self.rsp_sender.send((ip, data));
    }

    pub(super) fn should_send_raw_query(&mut self, ip: IpAddr, query_wait: Duration) -> bool {
        match self.doing_cache.entry(ip) {
            hash_map::Entry::Occupied(_) => false,
            hash_map::Entry::Vacant(cv) => {
                let timeout_key = self.doing_timeout_queue.insert(ip, query_wait);
                cv.insert(timeout_key);
                true
            }
        }
    }
}
