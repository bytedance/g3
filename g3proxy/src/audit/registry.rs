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

use once_cell::sync::Lazy;

use g3_types::metrics::MetricsName;

use super::Auditor;
use crate::audit::AuditorConfig;

static RUNTIME_AUDITOR_REGISTRY: Lazy<Mutex<HashMap<MetricsName, Arc<Auditor>>>> =
    Lazy::new(|| Mutex::new(HashMap::new()));

pub(super) fn add(name: MetricsName, auditor: Arc<Auditor>) {
    let mut ht = RUNTIME_AUDITOR_REGISTRY.lock().unwrap();
    if let Some(_old_group) = ht.insert(name, auditor) {}
}

pub(super) fn get(name: &MetricsName) -> Option<Arc<Auditor>> {
    let ht = RUNTIME_AUDITOR_REGISTRY.lock().unwrap();
    ht.get(name).map(Arc::clone)
}

pub(super) fn del(name: &MetricsName) {
    let mut ht = RUNTIME_AUDITOR_REGISTRY.lock().unwrap();
    if let Some(_old_auditor) = ht.remove(name) {}
}

pub(crate) fn get_names() -> HashSet<MetricsName> {
    let mut names = HashSet::new();
    let ht = RUNTIME_AUDITOR_REGISTRY.lock().unwrap();
    for key in ht.keys() {
        names.insert(key.clone());
    }
    names
}

pub(super) fn get_config(name: &MetricsName) -> Option<AuditorConfig> {
    let ht = RUNTIME_AUDITOR_REGISTRY.lock().unwrap();
    ht.get(name).map(|a| a.config.as_ref().clone())
}

pub(crate) fn get_or_insert_default(name: &MetricsName) -> Arc<Auditor> {
    let mut ht = RUNTIME_AUDITOR_REGISTRY.lock().unwrap();
    ht.entry(name.clone())
        .or_insert_with(|| Auditor::new_no_config(name))
        .clone()
}
