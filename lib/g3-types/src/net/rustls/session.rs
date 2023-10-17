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

use std::sync::Arc;

use rustls::server::{ServerSessionMemoryCache, StoresServerSessions};

pub struct RustlsTrickServerSessionCache {
    slots: [Arc<ServerSessionMemoryCache>; 16],
}

impl RustlsTrickServerSessionCache {
    pub fn new(each_size: usize) -> Self {
        RustlsTrickServerSessionCache {
            slots: [
                ServerSessionMemoryCache::new(each_size),
                ServerSessionMemoryCache::new(each_size),
                ServerSessionMemoryCache::new(each_size),
                ServerSessionMemoryCache::new(each_size),
                ServerSessionMemoryCache::new(each_size),
                ServerSessionMemoryCache::new(each_size),
                ServerSessionMemoryCache::new(each_size),
                ServerSessionMemoryCache::new(each_size),
                ServerSessionMemoryCache::new(each_size),
                ServerSessionMemoryCache::new(each_size),
                ServerSessionMemoryCache::new(each_size),
                ServerSessionMemoryCache::new(each_size),
                ServerSessionMemoryCache::new(each_size),
                ServerSessionMemoryCache::new(each_size),
                ServerSessionMemoryCache::new(each_size),
                ServerSessionMemoryCache::new(each_size),
            ],
        }
    }
}

impl StoresServerSessions for RustlsTrickServerSessionCache {
    fn put(&self, key: Vec<u8>, value: Vec<u8>) -> bool {
        let Some(c) = key.first() else {
            return false;
        };
        let id = *c & 0x0F;

        let Some(slot) = self.slots.get(id as usize) else {
            return false;
        };

        slot.put(key, value)
    }

    fn get(&self, key: &[u8]) -> Option<Vec<u8>> {
        let c = key.first()?;
        let id = *c & 0x0F;

        let slot = self.slots.get(id as usize)?;
        slot.get(key)
    }

    fn take(&self, key: &[u8]) -> Option<Vec<u8>> {
        let c = key.first()?;
        let id = *c & 0x0F;

        let slot = self.slots.get(id as usize)?;
        slot.take(key)
    }

    fn can_cache(&self) -> bool {
        true
    }
}
