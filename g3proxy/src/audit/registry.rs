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

use super::Auditor;
use crate::audit::AuditorConfig;

static RUNTIME_AUDITOR_REGISTRY: Lazy<Mutex<HashMap<String, Arc<Auditor>>>> =
    Lazy::new(|| Mutex::new(HashMap::new()));

pub(super) fn add(name: String, auditor: Arc<Auditor>) {
    let mut ht = RUNTIME_AUDITOR_REGISTRY.lock().unwrap();
    if let Some(_old_group) = ht.insert(name, auditor) {}
}

fn get(name: &str) -> Option<Arc<Auditor>> {
    let ht = RUNTIME_AUDITOR_REGISTRY.lock().unwrap();
    ht.get(name).map(Arc::clone)
}

pub(super) fn del(name: &str) {
    let mut ht = RUNTIME_AUDITOR_REGISTRY.lock().unwrap();
    if let Some(_old_auditor) = ht.remove(name) {}
}

pub(crate) fn get_names() -> HashSet<String> {
    let mut names = HashSet::new();
    let ht = RUNTIME_AUDITOR_REGISTRY.lock().unwrap();
    for key in ht.keys() {
        names.insert(key.to_string());
    }
    names
}

pub(super) fn get_config(name: &str) -> Option<AuditorConfig> {
    let ht = RUNTIME_AUDITOR_REGISTRY.lock().unwrap();
    if let Some(auditor) = ht.get(name) {
        let config = &*auditor.config;
        Some(config.clone())
    } else {
        None
    }
}

pub(super) fn reload_existed(name: &str, config: Option<AuditorConfig>) -> anyhow::Result<()> {
    let old_auditor = match get(name) {
        Some(auditor) => auditor,
        None => return Err(anyhow!("no auditor with name {name} found")),
    };

    let config = match config {
        Some(config) => config,
        None => {
            let config = &*old_auditor.config;
            config.clone()
        }
    };

    // the reload method is allowed to hold a registry lock
    // a tokio mutex is needed if we lock this await inside
    let group = old_auditor.reload(config);
    add(name.to_string(), group);
    Ok(())
}

pub(crate) fn get_or_insert_default(name: &str) -> Arc<Auditor> {
    let mut ht = RUNTIME_AUDITOR_REGISTRY.lock().unwrap();
    ht.entry(name.to_string())
        .or_insert_with(|| Auditor::new_no_config(name))
        .clone()
}
