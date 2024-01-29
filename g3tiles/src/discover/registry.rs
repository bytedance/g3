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
use std::sync::Mutex;

use anyhow::anyhow;
use once_cell::sync::Lazy;

use g3_types::metrics::MetricsName;

use super::ArcDiscover;
use crate::config::discover::AnyDiscoverConfig;

static RUNTIME_DISCOVER_REGISTRY: Lazy<Mutex<HashMap<MetricsName, ArcDiscover>>> =
    Lazy::new(|| Mutex::new(HashMap::new()));

pub(super) fn add(name: MetricsName, discover: ArcDiscover) {
    let mut ht = RUNTIME_DISCOVER_REGISTRY.lock().unwrap();
    if let Some(_old) = ht.insert(name, discover) {}
}

pub(crate) fn get(name: &MetricsName) -> Option<ArcDiscover> {
    let ht = RUNTIME_DISCOVER_REGISTRY.lock().unwrap();
    ht.get(name).cloned()
}

pub(super) fn del(name: &MetricsName) {
    let mut ht = RUNTIME_DISCOVER_REGISTRY.lock().unwrap();
    if let Some(_old) = ht.remove(name) {}
}

pub(crate) fn get_names() -> HashSet<MetricsName> {
    let mut names = HashSet::new();
    let ht = RUNTIME_DISCOVER_REGISTRY.lock().unwrap();
    for key in ht.keys() {
        names.insert(key.clone());
    }
    names
}

pub(super) fn get_config(name: &MetricsName) -> Option<AnyDiscoverConfig> {
    let ht = RUNTIME_DISCOVER_REGISTRY.lock().unwrap();
    ht.get(name).map(|g| g._clone_config())
}

pub(super) fn update_config_in_place(
    name: &MetricsName,
    config: AnyDiscoverConfig,
) -> anyhow::Result<()> {
    let ht = RUNTIME_DISCOVER_REGISTRY.lock().unwrap();
    if let Some(site) = ht.get(name) {
        site._update_config_in_place(config)
    } else {
        Err(anyhow!("no site with name {name} found"))
    }
}
