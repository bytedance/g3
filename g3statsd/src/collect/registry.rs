/*
 * Copyright 2025 ByteDance and/or its affiliates.
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
use std::sync::Mutex;

use anyhow::anyhow;
use foldhash::fast::FixedState;

use g3_types::metrics::NodeName;

use super::ArcCollector;
use crate::config::collector::AnyCollectorConfig;

static RUNTIME_COLLECTOR_REGISTRY: Mutex<HashMap<NodeName, ArcCollector, FixedState>> =
    Mutex::new(HashMap::with_hasher(FixedState::with_seed(0)));

pub(super) fn add(name: NodeName, collector: ArcCollector) {
    let mut ht = RUNTIME_COLLECTOR_REGISTRY.lock().unwrap();
    if let Some(old_collector) = ht.insert(name, collector) {
        old_collector._clean_to_offline();
    }
}

pub(super) fn del(name: &NodeName) {
    let mut ht = RUNTIME_COLLECTOR_REGISTRY.lock().unwrap();
    if let Some(old_collector) = ht.remove(name) {
        old_collector._clean_to_offline();
    }
}

pub(crate) fn foreach<F>(mut f: F)
where
    F: FnMut(&NodeName, &ArcCollector),
{
    let ht = RUNTIME_COLLECTOR_REGISTRY.lock().unwrap();
    for (name, collector) in ht.iter() {
        f(name, collector)
    }
}

pub(crate) fn get_names() -> HashSet<NodeName> {
    let mut names = HashSet::new();
    let ht = RUNTIME_COLLECTOR_REGISTRY.lock().unwrap();
    for name in ht.keys() {
        names.insert(name.clone());
    }
    names
}

pub(super) fn get_collector(name: &NodeName) -> Option<ArcCollector> {
    let ht = RUNTIME_COLLECTOR_REGISTRY.lock().unwrap();
    ht.get(name).cloned()
}

pub(super) fn get_config(name: &NodeName) -> Option<AnyCollectorConfig> {
    let ht = RUNTIME_COLLECTOR_REGISTRY.lock().unwrap();
    ht.get(name).map(|collect| collect._clone_config())
}

pub(super) fn reload_existed(
    name: &NodeName,
    config: Option<AnyCollectorConfig>,
) -> anyhow::Result<()> {
    let Some(old_collector) = get_collector(name) else {
        return Err(anyhow!("no collector with name {name} found"));
    };
    let config = config.unwrap_or_else(|| old_collector._clone_config());

    // the _reload method is allowed to hold a registry lock
    // a tokio mutex is needed if we lock this await inside
    let collector = old_collector._lock_safe_reload(config)?;

    add(name.clone(), collector);
    Ok(())
}

pub(crate) fn get_or_insert_default(name: &NodeName) -> ArcCollector {
    let mut ht = RUNTIME_COLLECTOR_REGISTRY.lock().unwrap();
    ht.entry(name.clone())
        .or_insert_with(|| super::discard::DiscardCollector::prepare_default(name))
        .clone()
}
