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

use std::collections::{BTreeMap, HashMap, HashSet};
use std::sync::{LazyLock, Mutex};

use anyhow::anyhow;

use g3_types::metrics::NodeName;

use super::{ArcIntegratedResolverHandle, BoxResolver};
use crate::config::resolver::AnyResolverConfig;

static RUNTIME_RESOLVER_REGISTRY: LazyLock<Mutex<HashMap<NodeName, BoxResolver>>> =
    LazyLock::new(|| Mutex::new(HashMap::new()));

pub(super) fn add(name: NodeName, resolver: BoxResolver) -> Option<BoxResolver> {
    let mut ht = RUNTIME_RESOLVER_REGISTRY.lock().unwrap();
    ht.insert(name, resolver)
}

pub(super) fn del(name: &NodeName) -> Option<BoxResolver> {
    let mut ht = RUNTIME_RESOLVER_REGISTRY.lock().unwrap();
    ht.remove(name)
}

pub(crate) fn foreach<F>(mut f: F)
where
    F: FnMut(&NodeName, &BoxResolver),
{
    let ht = RUNTIME_RESOLVER_REGISTRY.lock().unwrap();
    for (name, server) in ht.iter() {
        f(name, server)
    }
}

pub(crate) fn get_names() -> HashSet<NodeName> {
    let mut names = HashSet::new();
    let ht = RUNTIME_RESOLVER_REGISTRY.lock().unwrap();
    for name in ht.keys() {
        names.insert(name.clone());
    }
    names
}

pub(crate) fn get_handle(name: &NodeName) -> anyhow::Result<ArcIntegratedResolverHandle> {
    let ht = RUNTIME_RESOLVER_REGISTRY.lock().unwrap();
    match ht.get(name) {
        Some(resolver) => Ok(resolver.get_handle()),
        None => Err(anyhow!("no resolver with name {name} found")),
    }
}

pub(super) fn get_config(name: &NodeName) -> Option<AnyResolverConfig> {
    let ht = RUNTIME_RESOLVER_REGISTRY.lock().unwrap();
    ht.get(name).map(|resolver| resolver._clone_config())
}

pub(super) fn update_config(name: &NodeName, config: AnyResolverConfig) -> anyhow::Result<()> {
    let mut ht = RUNTIME_RESOLVER_REGISTRY.lock().unwrap();
    if ht.contains_key(name) {
        let mut dep_table = BTreeMap::new();
        if let Some(set) = config.dependent_resolver() {
            for name in set {
                if let Some(dr) = ht.get(&name) {
                    dep_table.insert(name, dr.get_handle());
                } else {
                    return Err(anyhow!("no dependency resolver with name {name} found"));
                }
            }
        }

        let resolver = ht.get_mut(name).unwrap();
        resolver._update_config(config, dep_table)
    } else {
        Err(anyhow!("no resolver with name {name} found"))
    }
}

pub(super) fn update_dependency(name: &NodeName, target: &NodeName) -> anyhow::Result<()> {
    let mut ht = RUNTIME_RESOLVER_REGISTRY.lock().unwrap();
    if let Some(target_resolver) = ht.get_mut(target) {
        let handle = target_resolver.get_handle();
        if let Some(resolver) = ht.get_mut(name) {
            resolver._update_dependent_handle(target, handle)
        } else {
            Err(anyhow!("no resolver with name {name} found"))
        }
    } else {
        Err(anyhow!("no resolver with name {name} found"))
    }
}
