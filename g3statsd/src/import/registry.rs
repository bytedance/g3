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
use std::sync::{Arc, Mutex};

use anyhow::anyhow;
use foldhash::fast::FixedState;

use g3_types::metrics::NodeName;

use super::ArcImporter;
use crate::config::importer::AnyImporterConfig;

static RUNTIME_IMPORTER_REGISTRY: Mutex<HashMap<NodeName, ArcImporter, FixedState>> =
    Mutex::new(HashMap::with_hasher(FixedState::with_seed(0)));

pub(super) fn add(name: NodeName, importer: ArcImporter) -> anyhow::Result<()> {
    let mut ht = RUNTIME_IMPORTER_REGISTRY
        .lock()
        .map_err(|e| anyhow!("failed to lock importer registry: {e}"))?;
    importer._start_runtime(&importer)?;
    if let Some(old_importer) = ht.insert(name, importer) {
        old_importer._abort_runtime();
    }
    Ok(())
}

pub(super) fn del(name: &NodeName) {
    let mut ht = RUNTIME_IMPORTER_REGISTRY.lock().unwrap();
    if let Some(old_importer) = ht.remove(name) {
        old_importer._abort_runtime();
    }
}

pub(crate) fn get_names() -> HashSet<NodeName> {
    let mut names = HashSet::new();
    let ht = RUNTIME_IMPORTER_REGISTRY.lock().unwrap();
    for name in ht.keys() {
        names.insert(name.clone());
    }
    names
}

pub(super) fn get_config(name: &NodeName) -> Option<AnyImporterConfig> {
    let ht = RUNTIME_IMPORTER_REGISTRY.lock().unwrap();
    ht.get(name).map(|importer| importer._clone_config())
}

pub(super) fn reload_no_respawn(name: &NodeName, config: AnyImporterConfig) -> anyhow::Result<()> {
    let mut ht = RUNTIME_IMPORTER_REGISTRY
        .lock()
        .map_err(|e| anyhow!("failed to lock importer registry: {e}"))?;
    let Some(old_importer) = ht.get(name) else {
        return Err(anyhow!("no importer with name {name} found"));
    };

    let importer = old_importer._reload_with_old_notifier(config)?;
    if let Some(_old_importer) = ht.insert(name.clone(), Arc::clone(&importer)) {
        // do not abort the runtime, as it's reused
    }
    importer._reload_config_notify_runtime();
    Ok(())
}

pub(crate) fn reload_only_collector(name: &NodeName) -> anyhow::Result<()> {
    let ht = RUNTIME_IMPORTER_REGISTRY
        .lock()
        .map_err(|e| anyhow!("failed to lock importer registry: {e}"))?;
    let Some(importer) = ht.get(name) else {
        return Err(anyhow!("no importer with name {name} found"));
    };

    importer._update_collector_in_place();
    Ok(())
}

pub(super) fn reload_and_respawn(name: &NodeName, config: AnyImporterConfig) -> anyhow::Result<()> {
    let mut ht = RUNTIME_IMPORTER_REGISTRY
        .lock()
        .map_err(|e| anyhow!("failed to lock importer registry: {e}"))?;
    let Some(old_importer) = ht.get(name) else {
        return Err(anyhow!("no importer with name {name} found"));
    };

    let importer = old_importer._reload_with_new_notifier(config)?;
    importer._start_runtime(&importer)?;
    if let Some(old_importer) = ht.insert(name.clone(), importer) {
        old_importer._abort_runtime();
    }
    Ok(())
}

pub(crate) fn foreach<F>(mut f: F)
where
    F: FnMut(&NodeName, &ArcImporter),
{
    let ht = RUNTIME_IMPORTER_REGISTRY.lock().unwrap();
    for (name, importer) in ht.iter() {
        f(name, importer)
    }
}

pub(crate) fn get_or_insert_default(name: &NodeName) -> ArcImporter {
    let mut ht = RUNTIME_IMPORTER_REGISTRY.lock().unwrap();
    ht.entry(name.clone())
        .or_insert_with(|| super::dummy::DummyImporter::prepare_default(name))
        .clone()
}
