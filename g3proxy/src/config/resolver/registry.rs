/*
 * SPDX-License-Identifier: Apache-2.0
 * Copyright 2023-2025 ByteDance and/or its affiliates.
 */

use std::collections::HashMap;
use std::sync::{Arc, Mutex};

use foldhash::fast::FixedState;

use g3_types::metrics::NodeName;

use super::AnyResolverConfig;

static INITIAL_RESOLVER_CONFIG_REGISTRY: Mutex<
    HashMap<NodeName, Arc<AnyResolverConfig>, FixedState>,
> = Mutex::new(HashMap::with_hasher(FixedState::with_seed(0)));

pub(crate) fn clear() {
    let mut ht = INITIAL_RESOLVER_CONFIG_REGISTRY.lock().unwrap();
    ht.clear();
}

pub(super) fn add(resolver: AnyResolverConfig) -> Option<AnyResolverConfig> {
    let name = resolver.name().clone();
    let resolver = Arc::new(resolver);
    let mut ht = INITIAL_RESOLVER_CONFIG_REGISTRY.lock().unwrap();
    ht.insert(name, resolver).map(|old| old.as_ref().clone())
}

pub(super) fn del(name: &NodeName) {
    let mut ht = INITIAL_RESOLVER_CONFIG_REGISTRY.lock().unwrap();
    ht.remove(name);
}

pub(super) fn get(name: &NodeName) -> Option<Arc<AnyResolverConfig>> {
    let ht = INITIAL_RESOLVER_CONFIG_REGISTRY.lock().unwrap();
    ht.get(name).cloned()
}

pub(super) fn get_all_names() -> Vec<NodeName> {
    let ht = INITIAL_RESOLVER_CONFIG_REGISTRY.lock().unwrap();
    ht.keys().cloned().collect()
}
