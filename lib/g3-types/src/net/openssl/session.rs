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

use std::collections::VecDeque;
use std::num::NonZeroUsize;
use std::sync::Mutex;

use anyhow::anyhow;
use lru::LruCache;
use openssl::ex_data::Index;
use openssl::ssl::{Ssl, SslContext, SslContextBuilder, SslSession, SslSessionCacheMode};

const SESSION_CACHE_DEFAULT_SITES_COUNT: NonZeroUsize = unsafe { NonZeroUsize::new_unchecked(128) };
const SESSION_CACHE_DEFAULT_EACH_CAPACITY: NonZeroUsize =
    unsafe { NonZeroUsize::new_unchecked(16) };

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum OpensslSessionCacheMethod {
    ForMany,
    ForOne,
    Builtin,
    Off,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(super) struct OpensslSessionCacheConfig {
    method: OpensslSessionCacheMethod,
    sites_count: NonZeroUsize,
    each_capacity: NonZeroUsize,
}

impl Default for OpensslSessionCacheConfig {
    fn default() -> Self {
        OpensslSessionCacheConfig {
            method: OpensslSessionCacheMethod::Builtin,
            sites_count: SESSION_CACHE_DEFAULT_SITES_COUNT,
            each_capacity: SESSION_CACHE_DEFAULT_EACH_CAPACITY,
        }
    }
}

impl OpensslSessionCacheConfig {
    pub(super) fn new_for_one() -> Self {
        OpensslSessionCacheConfig {
            method: OpensslSessionCacheMethod::ForOne,
            ..Default::default()
        }
    }

    pub(super) fn new_for_many() -> Self {
        OpensslSessionCacheConfig {
            method: OpensslSessionCacheMethod::ForMany,
            ..Default::default()
        }
    }

    pub(super) fn set_no_session_cache(&mut self) {
        self.method = OpensslSessionCacheMethod::Off;
    }

    pub(super) fn set_use_builtin_session_cache(&mut self) {
        self.method = OpensslSessionCacheMethod::Builtin;
    }

    pub(super) fn set_sites_count(&mut self, max: usize) {
        if let Some(max) = NonZeroUsize::new(max) {
            self.sites_count = max;
        }
    }

    pub(super) fn set_each_capacity(&mut self, cap: usize) {
        if let Some(cap) = NonZeroUsize::new(cap) {
            self.each_capacity = cap;
        }
    }

    pub(super) fn set_for_client(
        &self,
        ctx_builder: &mut SslContextBuilder,
    ) -> anyhow::Result<Option<OpensslTlsClientSessionCache>> {
        match self.method {
            OpensslSessionCacheMethod::ForMany => {
                let session_cache = OpensslTlsClientSessionCache::new()?;
                let caches = SessionCaches::for_many(self.sites_count, self.each_capacity.get());
                session_cache.add_to_context(ctx_builder, caches);
                Ok(Some(session_cache))
            }
            OpensslSessionCacheMethod::ForOne => {
                let session_cache = OpensslTlsClientSessionCache::new()?;
                let caches = SessionCaches::for_one(self.each_capacity.get());
                session_cache.add_to_context(ctx_builder, caches);
                Ok(Some(session_cache))
            }
            OpensslSessionCacheMethod::Builtin => {
                ctx_builder.set_session_cache_mode(SslSessionCacheMode::CLIENT);
                Ok(None)
            }
            OpensslSessionCacheMethod::Off => {
                ctx_builder.set_session_cache_mode(SslSessionCacheMode::OFF);
                Ok(None)
            }
        }
    }
}

struct ToOneCaches {
    capacity: usize,
    queue: VecDeque<SslSession>,
}

impl ToOneCaches {
    fn new(capacity: usize) -> Self {
        ToOneCaches {
            capacity,
            queue: VecDeque::new(),
        }
    }

    fn pop(&mut self) -> Option<SslSession> {
        self.queue.pop_front()
    }

    fn push(&mut self, s: SslSession) {
        self.queue.push_front(s);
        if self.queue.len() > self.capacity {
            self.queue.pop_back();
        }
    }
}

struct ToManyCaches {
    lru: LruCache<String, ToOneCaches, ahash::RandomState>,
    each_capacity: usize,
}

impl ToManyCaches {
    fn new(site_capacity: NonZeroUsize, each_capacity: usize) -> Self {
        ToManyCaches {
            lru: LruCache::with_hasher(site_capacity, ahash::RandomState::new()),
            each_capacity,
        }
    }

    fn get_or_insert_mut(&mut self, key: String) -> &mut ToOneCaches {
        self.lru
            .get_or_insert_mut(key, || ToOneCaches::new(self.each_capacity))
    }

    fn peek_mut(&mut self, key: &str) -> Option<&mut ToOneCaches> {
        self.lru.peek_mut(key)
    }
}

enum SessionCaches {
    One(Mutex<ToOneCaches>),
    Many(Mutex<ToManyCaches>),
}

impl SessionCaches {
    fn for_many(sites_count: NonZeroUsize, each_capacity: usize) -> Self {
        SessionCaches::Many(Mutex::new(ToManyCaches::new(sites_count, each_capacity)))
    }

    fn for_one(capacity: usize) -> Self {
        SessionCaches::One(Mutex::new(ToOneCaches::new(capacity)))
    }
}

#[derive(Clone, Copy)]
pub(super) struct OpensslTlsClientSessionCache {
    session_cache_index: Index<SslContext, SessionCaches>,
    session_key_index: Index<Ssl, String>,
}

impl OpensslTlsClientSessionCache {
    pub(super) fn new() -> anyhow::Result<Self> {
        let cache_index = SslContext::new_ex_index().map_err(anyhow::Error::new)?;
        let key_index = Ssl::new_ex_index().map_err(anyhow::Error::new)?;
        Ok(OpensslTlsClientSessionCache {
            session_cache_index: cache_index,
            session_key_index: key_index,
        })
    }

    fn add_to_context(&self, ctx_builder: &mut SslContextBuilder, caches: SessionCaches) {
        ctx_builder
            .set_session_cache_mode(SslSessionCacheMode::CLIENT | SslSessionCacheMode::NO_INTERNAL);

        let session_cache = *self;
        ctx_builder.set_new_session_callback(move |ssl, session| {
            if let Some(caches) = ssl.ssl_context().ex_data(session_cache.session_cache_index) {
                match caches {
                    SessionCaches::One(m) => m.lock().unwrap().push(session),
                    SessionCaches::Many(m) => {
                        if let Some(key) = ssl.ex_data(session_cache.session_key_index) {
                            m.lock()
                                .unwrap()
                                .get_or_insert_mut(key.clone())
                                .push(session);
                        }
                    }
                }
            }
        });

        ctx_builder.set_ex_data(session_cache.session_cache_index, caches);
    }

    pub(super) fn find_and_set_cache(
        &self,
        ssl: &mut Ssl,
        tls_name: &str,
        port: u16,
    ) -> anyhow::Result<()> {
        if let Some(caches) = ssl.ssl_context().ex_data(self.session_cache_index) {
            let session = match caches {
                SessionCaches::One(m) => {
                    let mut o = m.lock().unwrap();
                    o.pop()
                }
                SessionCaches::Many(m) => {
                    let key = format!("[{tls_name}]:{port}");
                    let session = m.lock().unwrap().peek_mut(&key).and_then(|m| m.pop());
                    ssl.set_ex_data(self.session_key_index, key);
                    session
                }
            };

            if let Some(s) = session {
                unsafe {
                    ssl.set_session(&s)
                        .map_err(|e| anyhow!("failed to set session: {e}"))?;
                }
            }
        }

        Ok(())
    }
}
