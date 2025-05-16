/*
 * SPDX-License-Identifier: Apache-2.0
 * Copyright 2023-2025 ByteDance and/or its affiliates.
 */

use std::collections::HashMap;
use std::sync::{Arc, Mutex};

use anyhow::anyhow;
use foldhash::fast::FixedState;

use g3_types::metrics::NodeName;

use super::AnyDiscoverConfig;

static INITIAL_DISCOVER_CONFIG_REGISTRY: Mutex<
    HashMap<NodeName, Arc<AnyDiscoverConfig>, FixedState>,
> = Mutex::new(HashMap::with_hasher(FixedState::with_seed(0)));

pub(crate) fn clear() {
    let mut ht = INITIAL_DISCOVER_CONFIG_REGISTRY.lock().unwrap();
    ht.clear();
}

pub(super) fn add(config: AnyDiscoverConfig, replace: bool) -> anyhow::Result<()> {
    let name = config.name().clone();
    let discover = Arc::new(config);
    let mut ht = INITIAL_DISCOVER_CONFIG_REGISTRY.lock().unwrap();
    if let Some(old) = ht.insert(name, discover) {
        if replace {
            Ok(())
        } else {
            Err(anyhow!(
                "discover with the same name {} is already existed",
                old.name()
            ))
        }
    } else {
        Ok(())
    }
}

pub(crate) fn get_all() -> Vec<Arc<AnyDiscoverConfig>> {
    let mut vec = Vec::new();
    let ht = INITIAL_DISCOVER_CONFIG_REGISTRY.lock().unwrap();
    for v in ht.values() {
        vec.push(Arc::clone(v));
    }
    vec
}
