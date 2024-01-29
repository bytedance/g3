/*
 * Copyright 2024 ByteDance and/or its affiliates.
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

use super::dummy_close::DummyCloseBackend;
use super::ArcBackend;
use crate::config::backend::AnyBackendConfig;

static RUNTIME_BACKEND_REGISTRY: Lazy<Mutex<HashMap<MetricsName, ArcBackend>>> =
    Lazy::new(|| Mutex::new(HashMap::new()));

pub(super) fn add(name: MetricsName, connector: ArcBackend) {
    let mut ht = RUNTIME_BACKEND_REGISTRY.lock().unwrap();
    if let Some(_old) = ht.insert(name, connector) {}
}

pub(super) fn get(name: &MetricsName) -> Option<ArcBackend> {
    let ht = RUNTIME_BACKEND_REGISTRY.lock().unwrap();
    ht.get(name).cloned()
}

pub(super) fn del(name: &MetricsName) {
    let mut ht = RUNTIME_BACKEND_REGISTRY.lock().unwrap();
    if let Some(_old) = ht.remove(name) {}
}

pub(crate) fn foreach<F>(mut f: F)
where
    F: FnMut(&MetricsName, &ArcBackend),
{
    let ht = RUNTIME_BACKEND_REGISTRY.lock().unwrap();
    for (name, backend) in ht.iter() {
        f(name, backend)
    }
}

pub(crate) fn get_names() -> HashSet<MetricsName> {
    let mut names = HashSet::new();
    let ht = RUNTIME_BACKEND_REGISTRY.lock().unwrap();
    for key in ht.keys() {
        names.insert(key.clone());
    }
    names
}

pub(super) fn get_config(name: &MetricsName) -> Option<AnyBackendConfig> {
    let ht = RUNTIME_BACKEND_REGISTRY.lock().unwrap();
    ht.get(name).map(|g| g._clone_config())
}

pub(super) fn update_config_in_place(
    name: &MetricsName,
    flags: u64,
    config: AnyBackendConfig,
) -> anyhow::Result<()> {
    let ht = RUNTIME_BACKEND_REGISTRY.lock().unwrap();
    if let Some(backend) = ht.get(name) {
        backend._update_config_in_place(flags, config)
    } else {
        Err(anyhow!("no backend with name {name} found"))
    }
}

pub(super) async fn reload_existed(
    name: &MetricsName,
    config: Option<AnyBackendConfig>,
) -> anyhow::Result<()> {
    let Some(old_backend) = get(name) else {
        return Err(anyhow!("no backend with name {name} found"));
    };
    let config = config.unwrap_or_else(|| old_backend._clone_config());

    // the _reload method is allowed to hold a registry lock
    // a tokio mutex is needed if we lock this await inside
    let backend = old_backend._lock_safe_reload(config).await?;

    add(name.clone(), backend);
    Ok(())
}

pub(crate) fn get_or_insert_default(name: &MetricsName) -> ArcBackend {
    let mut ht = RUNTIME_BACKEND_REGISTRY.lock().unwrap();
    ht.entry(name.clone())
        .or_insert_with(|| DummyCloseBackend::prepare_default(name))
        .clone()
}
