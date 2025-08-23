/*
 * SPDX-License-Identifier: Apache-2.0
 * Copyright 2023-2025 ByteDance and/or its affiliates.
 */

use std::collections::HashMap;
use std::sync::{Arc, Mutex};

use foldhash::fast::FixedState;

use g3_types::metrics::NodeName;

use super::AnyKeyStoreConfig;

static INITIAL_STORE_CONFIG_REGISTRY: Mutex<HashMap<NodeName, Arc<AnyKeyStoreConfig>, FixedState>> =
    Mutex::new(HashMap::with_hasher(FixedState::with_seed(0)));

pub(crate) fn clear() {
    let mut ht = INITIAL_STORE_CONFIG_REGISTRY.lock().unwrap();
    ht.clear();
}

pub(super) fn add(store: AnyKeyStoreConfig) -> Option<AnyKeyStoreConfig> {
    let name = store.name().clone();
    let escaper = Arc::new(store);
    let mut ht = INITIAL_STORE_CONFIG_REGISTRY.lock().unwrap();
    ht.insert(name, escaper).map(|old| old.as_ref().clone())
}

pub(crate) fn get_all() -> Vec<Arc<AnyKeyStoreConfig>> {
    let mut vec = Vec::new();
    let ht = INITIAL_STORE_CONFIG_REGISTRY.lock().unwrap();
    for v in ht.values() {
        vec.push(Arc::clone(v));
    }
    vec
}
