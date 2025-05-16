/*
 * SPDX-License-Identifier: Apache-2.0
 * Copyright 2024-2025 ByteDance and/or its affiliates.
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
