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

use std::num::NonZeroUsize;
use std::sync::{Arc, Mutex};

use lru::LruCache;
use openssl::error::ErrorStack;
use openssl::ex_data::Index;
use openssl::hash::{Hasher, MessageDigest};
use openssl::ssl::{SslContext, SslContextBuilder, SslSession, SslSessionCacheMode};
use openssl::x509::{X509NameRef, X509Ref};

pub struct OpensslSessionIdContext {
    hasher: Hasher,
}

impl OpensslSessionIdContext {
    pub fn new() -> Result<Self, ErrorStack> {
        let hasher = Hasher::new(MessageDigest::sha1())?;
        Ok(OpensslSessionIdContext { hasher })
    }

    pub fn add_text(&mut self, s: &str) -> Result<(), ErrorStack> {
        self.hasher.update(s.as_bytes())
    }

    pub fn add_cert(&mut self, cert: &X509Ref) -> Result<(), ErrorStack> {
        let cert_digest = cert.digest(MessageDigest::sha1())?;
        self.hasher.update(cert_digest.as_ref())
    }

    pub fn add_ca_subject(&mut self, name: &X509NameRef) -> Result<(), ErrorStack> {
        let name_digest = name.digest(MessageDigest::sha1())?;
        self.hasher.update(name_digest.as_ref())
    }

    pub fn build_set(mut self, ssl_builder: &mut SslContextBuilder) -> Result<(), ErrorStack> {
        let digest = self.hasher.finish()?;
        ssl_builder.set_session_id_context(digest.as_ref())
    }
}

struct CacheSlot {
    local: Mutex<LruCache<Vec<u8>, SslSession, ahash::RandomState>>,
}

impl CacheSlot {
    fn new(size: NonZeroUsize) -> Self {
        CacheSlot {
            local: Mutex::new(LruCache::with_hasher(size, ahash::RandomState::new())),
        }
    }
}

struct SessionCache {
    slots: [CacheSlot; 16],
}

impl Default for SessionCache {
    fn default() -> Self {
        SessionCache::new(256)
    }
}

impl SessionCache {
    fn new(each_size: usize) -> Self {
        let each_size = NonZeroUsize::new(each_size)
            .unwrap_or_else(|| unsafe { NonZeroUsize::new_unchecked(256) });
        SessionCache {
            slots: [
                CacheSlot::new(each_size),
                CacheSlot::new(each_size),
                CacheSlot::new(each_size),
                CacheSlot::new(each_size),
                CacheSlot::new(each_size),
                CacheSlot::new(each_size),
                CacheSlot::new(each_size),
                CacheSlot::new(each_size),
                CacheSlot::new(each_size),
                CacheSlot::new(each_size),
                CacheSlot::new(each_size),
                CacheSlot::new(each_size),
                CacheSlot::new(each_size),
                CacheSlot::new(each_size),
                CacheSlot::new(each_size),
                CacheSlot::new(each_size),
            ],
        }
    }

    fn push(&self, session: SslSession) {
        let key = session.id();
        let Some(c) = key.first() else {
            return;
        };
        let id = *c & 0x0F;
        let slot = unsafe { self.slots.get_unchecked(id as usize) };

        let mut cache = slot.local.lock().unwrap();
        cache.push(key.to_vec(), session);
    }

    fn pop(&self, key: &[u8]) -> Option<SslSession> {
        let c = key.first()?;
        let id = *c & 0x0F;
        let slot = unsafe { self.slots.get_unchecked(id as usize) };

        let mut cache = slot.local.lock().unwrap();
        cache.pop(key)
    }
}

#[derive(Clone)]
pub struct OpensslServerSessionCache {
    cache: Arc<SessionCache>,
    session_cache_index: Index<SslContext, Arc<SessionCache>>,
}

impl OpensslServerSessionCache {
    pub fn new(each_size: usize) -> anyhow::Result<Self> {
        let cache = SessionCache::new(each_size);
        let cache_index = SslContext::new_ex_index().map_err(anyhow::Error::new)?;
        Ok(OpensslServerSessionCache {
            cache: Arc::new(cache),
            session_cache_index: cache_index,
        })
    }

    pub fn add_to_context(&self, ctx_builder: &mut SslContextBuilder) {
        ctx_builder.set_session_cache_mode(SslSessionCacheMode::SERVER);

        let session_cache_index = self.session_cache_index;
        ctx_builder.set_new_session_callback(move |ssl, session| {
            if let Some(cache) = ssl.ssl_context().ex_data(session_cache_index) {
                cache.push(session);
            }
        });

        let session_cache_index = self.session_cache_index;
        unsafe {
            ctx_builder.set_get_session_callback(move |ssl, id| {
                if let Some(cache) = ssl.ssl_context().ex_data(session_cache_index) {
                    cache.pop(id)
                } else {
                    None
                }
            })
        }

        let session_cache_index = self.session_cache_index;
        ctx_builder.set_remove_session_callback(move |ctx, session| {
            if let Some(cache) = ctx.ex_data(session_cache_index) {
                cache.pop(session.id());
            }
        });

        ctx_builder.set_ex_data(self.session_cache_index, self.cache.clone());
    }
}
