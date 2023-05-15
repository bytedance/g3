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

use g3_types::metrics::MetricsName;

use super::dummy_deny::DummyDenyEscaper;
use super::ArcEscaper;
use crate::config::escaper::AnyEscaperConfig;

static RUNTIME_ESCAPER_REGISTRY: Lazy<Mutex<HashMap<MetricsName, ArcEscaper>>> =
    Lazy::new(|| Mutex::new(HashMap::new()));

pub(super) fn add(name: MetricsName, escaper: ArcEscaper) {
    let mut ht = RUNTIME_ESCAPER_REGISTRY.lock().unwrap();
    if let Some(_old_escaper) = ht.insert(name, escaper) {}
}

fn get(name: &MetricsName) -> Option<ArcEscaper> {
    let ht = RUNTIME_ESCAPER_REGISTRY.lock().unwrap();
    ht.get(name).map(Arc::clone)
}

pub(super) fn del(name: &MetricsName) {
    let mut ht = RUNTIME_ESCAPER_REGISTRY.lock().unwrap();
    if let Some(_old_escaper) = ht.remove(name) {}
}

pub(crate) fn foreach<F>(mut f: F)
where
    F: FnMut(&MetricsName, &ArcEscaper),
{
    let ht = RUNTIME_ESCAPER_REGISTRY.lock().unwrap();
    for (name, escaper) in ht.iter() {
        f(name, escaper)
    }
}

pub(crate) fn get_names() -> HashSet<MetricsName> {
    let mut names = HashSet::new();
    let ht = RUNTIME_ESCAPER_REGISTRY.lock().unwrap();
    for key in ht.keys() {
        names.insert(key.clone());
    }
    names
}

pub(super) fn get_escaper(name: &MetricsName) -> Option<ArcEscaper> {
    let ht = RUNTIME_ESCAPER_REGISTRY.lock().unwrap();
    ht.get(name).map(Arc::clone)
}

pub(super) fn get_config(name: &MetricsName) -> Option<AnyEscaperConfig> {
    let ht = RUNTIME_ESCAPER_REGISTRY.lock().unwrap();
    ht.get(name).map(|escaper| escaper._clone_config())
}

pub(super) fn update_config_in_place(
    name: &MetricsName,
    flags: u64,
    config: AnyEscaperConfig,
) -> anyhow::Result<()> {
    let ht = RUNTIME_ESCAPER_REGISTRY.lock().unwrap();
    if let Some(escaper) = ht.get(name) {
        escaper._update_config_in_place(flags, config)
    } else {
        Err(anyhow!("no escaper with name {name} found"))
    }
}

pub(super) async fn reload_existed(
    name: &MetricsName,
    config: Option<AnyEscaperConfig>,
) -> anyhow::Result<()> {
    let Some(old_escaper) = get(name) else {
        return Err(anyhow!("no escaper with name {name} found"));
    };
    let config = config.unwrap_or_else(|| old_escaper._clone_config());

    // the _reload method is allowed to hold a registry lock
    // a tokio mutex is needed if we lock this await inside
    let escaper = old_escaper._lock_safe_reload(config).await?;

    add(name.clone(), escaper);
    Ok(())
}

pub(crate) fn get_or_insert_default(name: &MetricsName) -> ArcEscaper {
    let mut ht = RUNTIME_ESCAPER_REGISTRY.lock().unwrap();
    ht.entry(name.clone())
        .or_insert_with(|| DummyDenyEscaper::prepare_default(name))
        .clone()
}
