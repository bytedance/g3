/*
 * SPDX-License-Identifier: Apache-2.0
 * Copyright 2023-2025 ByteDance and/or its affiliates.
 */

use std::collections::{HashMap, HashSet};
use std::sync::{Arc, Mutex};

use foldhash::fast::FixedState;

use g3_types::metrics::NodeName;

use super::UserGroup;
use crate::config::auth::UserGroupConfig;

static RUNTIME_USER_GROUP_REGISTRY: Mutex<HashMap<NodeName, Arc<UserGroup>, FixedState>> =
    Mutex::new(HashMap::with_hasher(FixedState::with_seed(0)));

fn foreach<F>(mut f: F)
where
    F: FnMut(&NodeName, &Arc<UserGroup>),
{
    let ht = RUNTIME_USER_GROUP_REGISTRY.lock().unwrap();
    for (name, user_group) in ht.iter() {
        f(name, user_group)
    }
}

pub(crate) fn get_all_groups() -> Vec<Arc<UserGroup>> {
    let mut groups = Vec::new();
    foreach(|_, group| {
        groups.push(Arc::clone(group));
    });
    groups
}

pub(super) fn add(name: NodeName, group: Arc<UserGroup>) {
    let mut ht = RUNTIME_USER_GROUP_REGISTRY.lock().unwrap();
    if let Some(old_group) = ht.insert(name, group) {
        old_group.stop_fetch_job();
    }
}

pub(super) fn get(name: &NodeName) -> Option<Arc<UserGroup>> {
    let ht = RUNTIME_USER_GROUP_REGISTRY.lock().unwrap();
    ht.get(name).cloned()
}

pub(super) fn del(name: &NodeName) {
    let mut ht = RUNTIME_USER_GROUP_REGISTRY.lock().unwrap();
    if let Some(old_group) = ht.remove(name) {
        old_group.stop_fetch_job();
    }
}

pub(crate) fn get_names() -> HashSet<NodeName> {
    let mut names = HashSet::new();
    let ht = RUNTIME_USER_GROUP_REGISTRY.lock().unwrap();
    for key in ht.keys() {
        names.insert(key.clone());
    }
    names
}

pub(super) fn get_config(name: &NodeName) -> Option<UserGroupConfig> {
    let ht = RUNTIME_USER_GROUP_REGISTRY.lock().unwrap();
    ht.get(name).map(|g| g.config.as_ref().clone())
}

pub(crate) fn get_or_insert_default(name: &NodeName) -> Arc<UserGroup> {
    let mut ht = RUNTIME_USER_GROUP_REGISTRY.lock().unwrap();
    ht.entry(name.clone())
        .or_insert_with(|| UserGroup::new_no_config(name))
        .clone()
}
