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

use std::cell::RefCell;

use ahash::AHashMap;
use anyhow::anyhow;
use openssl::hash::MessageDigest;
use openssl::pkey::{PKey, Private};

use g3_tls_cert::ext::X509Pubkey;

mod ops;
pub use ops::{load_all, reload_all};

mod registry;

thread_local! {
    static GLOBAL_SKI_MAP: RefCell<AHashMap<Vec<u8>, PKey<Private>>> = RefCell::new(AHashMap::new());
}

pub(crate) fn add_global(key: PKey<Private>) -> anyhow::Result<()> {
    let x =
        X509Pubkey::from_pubkey(&key).map_err(|e| anyhow!("failed to build X509 PUBKEY: {e}"))?;
    let encoded = x
        .encoded_bytes()
        .map_err(|e| anyhow!("failed to get encoded X509 PUBKEY bytes: {e}"))?;
    let ski = openssl::hash::hash(MessageDigest::sha1(), encoded)
        .map_err(|e| anyhow!("failed to calculate SKI value: {e}"))?;

    GLOBAL_SKI_MAP.with(|cell| {
        let mut map = cell.borrow_mut();
        map.insert(ski.to_vec(), key);
    });
    Ok(())
}

pub(crate) fn get_by_ski(ski: &[u8]) -> Option<PKey<Private>> {
    GLOBAL_SKI_MAP.with(|cell| {
        let map = cell.borrow();
        map.get(ski).cloned()
    })
}
