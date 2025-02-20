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

use std::sync::{Arc, RwLock};

use arc_swap::ArcSwap;
use rustc_hash::{FxBuildHasher, FxHashMap};

use super::TicketKeyName;

pub const TICKET_AES_KEY_LENGTH: usize = 32;
pub const TICKET_AES_IV_LENGTH: usize = 16;
pub const TICKET_HMAC_KEY_LENGTH: usize = 16;

pub trait RollingTicketKey: Sized {
    fn new_random(lifetime: u32) -> anyhow::Result<Self>;
    fn name(&self) -> TicketKeyName;
    fn lifetime(&self) -> u32;
}

pub struct RollingTicketer<K: RollingTicketKey> {
    dec_keys: RwLock<FxHashMap<TicketKeyName, Arc<K>>>,
    pub(crate) enc_key: ArcSwap<K>,
}

impl<K: RollingTicketKey> RollingTicketer<K> {
    pub fn new(initial_key: K) -> Self {
        let key = Arc::new(initial_key);
        let dec_keys = RwLock::new(FxHashMap::with_capacity_and_hasher(4, FxBuildHasher));
        let ticketer = RollingTicketer {
            dec_keys,
            enc_key: ArcSwap::new(key.clone()),
        };
        ticketer.add_decrypt_key(key);
        ticketer
    }

    pub fn get_decrypt_key(&self, name: &[u8]) -> Option<Arc<K>> {
        let Ok(key_name) = TicketKeyName::try_from(name) else {
            return None;
        };
        self.dec_keys.read().unwrap().get(&key_name).cloned()
    }

    pub fn add_decrypt_key(&self, key: Arc<K>) {
        let name = key.name();
        self.dec_keys.write().unwrap().insert(name, key);
    }

    pub fn del_decrypt_key(&self, name: TicketKeyName) {
        self.dec_keys.write().unwrap().remove(&name);
    }

    pub fn encrypt_key(&self) -> Arc<K> {
        self.enc_key.load_full()
    }

    pub fn set_encrypt_key(&self, key: Arc<K>) {
        self.enc_key.store(key);
    }
}
