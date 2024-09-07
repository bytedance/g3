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

use ahash::AHashMap;
use arc_swap::ArcSwap;

use super::TicketKeyName;

pub const TICKET_AES_KEY_LENGTH: usize = 32;
pub const TICKET_AES_IV_LENGTH: usize = 16;
pub const TICKET_HMAC_KEY_LENGTH: usize = 16;

pub trait RollingTicketKey: Sized {
    fn new_random(lifetime: u32) -> anyhow::Result<Self>;
    fn name(&self) -> &TicketKeyName;
    fn lifetime(&self) -> u32;
}

pub struct RollingTicketer<K: RollingTicketKey> {
    dec_keys: RwLock<AHashMap<TicketKeyName, Arc<K>>>,
    pub(crate) enc_key: ArcSwap<K>,
}

impl<K: RollingTicketKey> RollingTicketer<K> {
    pub fn get_decrypt_key(&self, name: &[u8]) -> Option<Arc<K>> {
        let Ok(key_name) = TicketKeyName::try_from(name) else {
            return None;
        };
        self.dec_keys.read().unwrap().get(&key_name).cloned()
    }

    pub fn add_decrypt_key(&self, key: Arc<K>) {
        let name = *key.name();
        self.dec_keys.write().unwrap().insert(name, key);
    }

    pub fn set_encrypt_key(&self, key: Arc<K>) {
        self.add_decrypt_key(key.clone());
        self.enc_key.store(key);
    }
}
