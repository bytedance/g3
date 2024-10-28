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

use std::mem;
use std::sync::Mutex;
use std::time::Duration;

use rustc_hash::FxHashMap;
use tokio::sync::oneshot;
use tokio::time::Instant;

use crate::module::keyless::{
    KeylessHeader, KeylessInternalErrorResponse, KeylessResponse, KeylessUpstreamResponse,
};

pub(super) struct CachedValue {
    send_started: Instant,
    req_header: KeylessHeader,
    rsp_sender: oneshot::Sender<KeylessResponse>,
}

impl CachedValue {
    pub(super) fn new(
        req_header: KeylessHeader,
        rsp_sender: oneshot::Sender<KeylessResponse>,
    ) -> Self {
        CachedValue {
            send_started: Instant::now(),
            req_header,
            rsp_sender,
        }
    }

    pub(super) fn elapsed(&self) -> Duration {
        self.send_started.elapsed()
    }

    pub(super) fn send_upstream_rsp(self, rsp: KeylessUpstreamResponse) -> Result<(), ()> {
        self.rsp_sender
            .send(KeylessResponse::Upstream(rsp.refresh(self.req_header)))
            .map_err(|_| ())
    }

    pub(super) fn send_internal_error(self) {
        let _ = self
            .rsp_sender
            .send(KeylessResponse::Local(KeylessInternalErrorResponse::new(
                self.req_header,
            )));
    }
}

#[derive(Default)]
struct StreamState {
    cur_cache: FxHashMap<u32, CachedValue>,
    old_cache: FxHashMap<u32, CachedValue>,
}

#[derive(Default)]
pub(super) struct StreamSharedState {
    inner: Mutex<StreamState>,
}

impl StreamSharedState {
    pub(super) fn add_request(
        &self,
        id: u32,
        orig_header: KeylessHeader,
        rsp_sender: oneshot::Sender<KeylessResponse>,
    ) {
        let mut inner = self.inner.lock().unwrap();
        inner
            .cur_cache
            .insert(id, CachedValue::new(orig_header, rsp_sender));
    }

    pub(super) fn fetch_request(&self, id: u32) -> Option<CachedValue> {
        let mut inner = self.inner.lock().unwrap();
        inner
            .cur_cache
            .remove(&id)
            .or_else(|| inner.old_cache.remove(&id))
    }

    pub(super) fn has_pending(&self) -> bool {
        let inner = self.inner.lock().unwrap();
        !(inner.cur_cache.is_empty() && inner.old_cache.is_empty())
    }

    pub(super) fn drain<F>(&self, handle: F)
    where
        F: Fn(u32, CachedValue),
    {
        let mut inner = self.inner.lock().unwrap();
        inner.cur_cache.drain().for_each(|(id, v)| handle(id, v));
        inner.old_cache.drain().for_each(|(id, v)| handle(id, v));
    }

    pub(super) fn rotate_timeout<F>(&self, handle: F)
    where
        F: Fn(u32, CachedValue),
    {
        let mut old_ht = {
            let mut inner = self.inner.lock().unwrap();
            let cur_ht = mem::take(&mut inner.cur_cache);
            mem::replace(&mut inner.old_cache, cur_ht)
        };
        old_ht.drain().for_each(|(id, v)| handle(id, v));
    }
}
