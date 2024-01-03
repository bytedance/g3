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
use std::sync::Mutex;

use lru::LruCache;
use rustls::server::StoresServerSessions;

struct CacheSlot {
    local: Mutex<LruCache<Vec<u8>, Vec<u8>, ahash::RandomState>>,
}

impl CacheSlot {
    fn new(size: NonZeroUsize) -> Self {
        CacheSlot {
            local: Mutex::new(LruCache::with_hasher(size, ahash::RandomState::new())),
        }
    }
}

pub struct RustlsServerSessionCache {
    slots: [CacheSlot; 16],
}

impl Default for RustlsServerSessionCache {
    fn default() -> Self {
        RustlsServerSessionCache::new(256)
    }
}

impl RustlsServerSessionCache {
    pub fn new(each_size: usize) -> Self {
        let each_size = NonZeroUsize::new(each_size)
            .unwrap_or_else(|| unsafe { NonZeroUsize::new_unchecked(256) });
        RustlsServerSessionCache {
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
}

impl StoresServerSessions for RustlsServerSessionCache {
    fn put(&self, key: Vec<u8>, value: Vec<u8>) -> bool {
        let Some(c) = key.first() else {
            return false;
        };
        let id = *c & 0x0F;
        let slot = unsafe { self.slots.get_unchecked(id as usize) };

        let mut cache = slot.local.lock().unwrap();
        cache.put(key, value);
        true
    }

    fn get(&self, key: &[u8]) -> Option<Vec<u8>> {
        let c = key.first()?;
        let id = *c & 0x0F;
        let slot = unsafe { self.slots.get_unchecked(id as usize) };

        let mut cache = slot.local.lock().unwrap();
        cache.get(key).cloned()
    }

    fn take(&self, key: &[u8]) -> Option<Vec<u8>> {
        let c = key.first()?;
        let id = *c & 0x0F;
        let slot = unsafe { self.slots.get_unchecked(id as usize) };

        let mut cache = slot.local.lock().unwrap();
        cache.pop(key)
    }

    fn can_cache(&self) -> bool {
        true
    }
}
