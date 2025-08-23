/*
 * SPDX-License-Identifier: Apache-2.0
 * Copyright 2023-2025 ByteDance and/or its affiliates.
 */

use std::collections::HashMap;
use std::sync::RwLock;

use anyhow::anyhow;
use foldhash::fast::FixedState;
use openssl::pkey::{PKey, Private};

use g3_tls_cert::ext::PublicKeyExt;

mod ops;
pub use ops::{load_all, reload_all};

mod registry;

static GLOBAL_SKI_MAP: RwLock<HashMap<Vec<u8>, PKey<Private>, FixedState>> =
    RwLock::new(HashMap::with_hasher(FixedState::with_seed(0)));

pub(crate) fn add_global(key: PKey<Private>) -> anyhow::Result<()> {
    let ski = key.ski().map_err(|e| anyhow!("failed to get SKI: {e}"))?;
    let mut map = GLOBAL_SKI_MAP
        .write()
        .map_err(|e| anyhow!("failed to get global ski_map: {e}"))?;
    map.insert(ski.to_vec(), key);
    Ok(())
}

pub(crate) fn get_all_ski() -> Vec<Vec<u8>> {
    let map = GLOBAL_SKI_MAP.read().unwrap();
    map.keys().map(|v| v.to_vec()).collect()
}

pub(crate) fn get_by_ski(ski: &[u8]) -> Option<PKey<Private>> {
    let map = GLOBAL_SKI_MAP.read().unwrap();
    map.get(ski).cloned()
}
