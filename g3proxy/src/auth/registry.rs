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

use std::collections::{HashMap, HashSet};
use std::sync::{Arc, Mutex};

use anyhow::anyhow;
use once_cell::sync::Lazy;

use super::UserGroup;
use crate::config::auth::UserGroupConfig;

static RUNTIME_USER_GROUP_REGISTRY: Lazy<Mutex<HashMap<String, Arc<UserGroup>>>> =
    Lazy::new(|| Mutex::new(HashMap::new()));

fn foreach<F>(mut f: F)
where
    F: FnMut(&str, &Arc<UserGroup>),
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

pub(super) fn add(name: String, group: Arc<UserGroup>) {
    let mut ht = RUNTIME_USER_GROUP_REGISTRY.lock().unwrap();
    if let Some(_old_group) = ht.insert(name, group) {}
}

fn get(name: &str) -> Option<Arc<UserGroup>> {
    let ht = RUNTIME_USER_GROUP_REGISTRY.lock().unwrap();
    ht.get(name).map(Arc::clone)
}

pub(super) fn del(name: &str) {
    let mut ht = RUNTIME_USER_GROUP_REGISTRY.lock().unwrap();
    if let Some(_old_group) = ht.remove(name) {}
}

pub(crate) fn get_names() -> HashSet<String> {
    let mut names = HashSet::new();
    let ht = RUNTIME_USER_GROUP_REGISTRY.lock().unwrap();
    for key in ht.keys() {
        names.insert(key.to_string());
    }
    names
}

pub(super) fn get_config(name: &str) -> Option<UserGroupConfig> {
    let ht = RUNTIME_USER_GROUP_REGISTRY.lock().unwrap();
    if let Some(group) = ht.get(name) {
        let config = &*group.config;
        Some(config.clone())
    } else {
        None
    }
}

pub(super) fn reload_existed(name: &str, config: Option<UserGroupConfig>) -> anyhow::Result<()> {
    let old_group = match get(name) {
        Some(group) => group,
        None => return Err(anyhow!("no user group with name {name} found")),
    };

    let config = match config {
        Some(config) => config,
        None => {
            let config = &*old_group.config;
            config.clone()
        }
    };

    // the reload method is allowed to hold a registry lock
    // a tokio mutex is needed if we lock this await inside
    let group = old_group.reload(config)?;
    add(name.to_string(), group);
    Ok(())
}

pub(crate) fn get_or_insert_default(name: &str) -> Arc<UserGroup> {
    let mut ht = RUNTIME_USER_GROUP_REGISTRY.lock().unwrap();
    ht.entry(name.to_string())
        .or_insert_with(|| UserGroup::new_no_config(name))
        .clone()
}
