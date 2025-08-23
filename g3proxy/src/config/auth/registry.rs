/*
 * SPDX-License-Identifier: Apache-2.0
 * Copyright 2023-2025 ByteDance and/or its affiliates.
 */

use std::collections::HashMap;
use std::sync::{Arc, Mutex};

use anyhow::anyhow;
use foldhash::fast::FixedState;

use super::UserGroupConfig;

static INITIAL_USER_GROUP_CONFIG_REGISTRY: Mutex<
    HashMap<String, Arc<UserGroupConfig>, FixedState>,
> = Mutex::new(HashMap::with_hasher(FixedState::with_seed(0)));

pub(crate) fn clear() {
    let mut ht = INITIAL_USER_GROUP_CONFIG_REGISTRY.lock().unwrap();
    ht.clear();
}

pub(super) fn add(group: UserGroupConfig, replace: bool) -> anyhow::Result<()> {
    let name = group.name().to_string();
    let group = Arc::new(group);
    let mut ht = INITIAL_USER_GROUP_CONFIG_REGISTRY.lock().unwrap();
    if let Some(old) = ht.insert(name, group) {
        if replace {
            Ok(())
        } else {
            Err(anyhow!(
                "user group with the same name {} is already existed",
                old.name()
            ))
        }
    } else {
        Ok(())
    }
}

pub(crate) fn get_all() -> Vec<Arc<UserGroupConfig>> {
    let mut vec = Vec::new();
    let ht = INITIAL_USER_GROUP_CONFIG_REGISTRY.lock().unwrap();
    for v in ht.values() {
        vec.push(Arc::clone(v));
    }
    vec
}
