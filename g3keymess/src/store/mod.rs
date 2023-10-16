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
use openssl::pkey::{PKey, Private};

use g3_tls_cert::ext::PublicKeyExt;

mod ops;
pub use ops::{load_all, reload_all};

mod registry;

thread_local! {
    static GLOBAL_SKI_MAP: RefCell<AHashMap<Vec<u8>, PKey<Private>>> = RefCell::new(AHashMap::new());
}

pub(crate) fn add_global(key: PKey<Private>) -> anyhow::Result<()> {
    let ski = key.ski().map_err(|e| anyhow!("failed to get SKI: {e}"))?;
    GLOBAL_SKI_MAP.with_borrow_mut(|map| {
        map.insert(ski.to_vec(), key);
    });

    Ok(())
}

pub(crate) fn get_all_ski() -> Vec<Vec<u8>> {
    GLOBAL_SKI_MAP.with_borrow(|map| map.keys().map(|v| v.to_vec()).collect())
}

pub(crate) fn get_by_ski(ski: &[u8]) -> Option<PKey<Private>> {
    GLOBAL_SKI_MAP.with_borrow(|map| map.get(ski).cloned())
}
