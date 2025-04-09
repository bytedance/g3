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

use super::ArcExporter;
use crate::config::exporter::AnyExporterConfig;

static RUNTIME_EXPORTER_REGISTRY: Mutex<HashMap<NodeName, ArcExporter, FixedState>> =
    Mutex::new(HashMap::with_hasher(FixedState::with_seed(0)));

pub(super) fn add(name: NodeName, exporter: ArcExporter) {
    let mut ht = RUNTIME_EXPORTER_REGISTRY.lock().unwrap();
    if let Some(old_exporter) = ht.insert(name, exporter) {
        old_exporter._clean_to_offline();
    }
}

pub(super) fn del(name: &NodeName) {
    let mut ht = RUNTIME_EXPORTER_REGISTRY.lock().unwrap();
    if let Some(old_exporter) = ht.remove(name) {
        old_exporter._clean_to_offline();
    }
}

pub(crate) fn get_names() -> HashSet<NodeName> {
    let ht = RUNTIME_EXPORTER_REGISTRY.lock().unwrap();
    ht.keys().cloned().collect()
}

pub(super) fn get_config(name: &NodeName) -> Option<AnyExporterConfig> {
    let ht = RUNTIME_EXPORTER_REGISTRY.lock().unwrap();
    ht.get(name).map(|exporter| exporter._clone_config())
}

pub(super) fn reload_existed(
    name: &NodeName,
    config: Option<AnyExporterConfig>,
) -> anyhow::Result<()> {
    let mut ht = RUNTIME_EXPORTER_REGISTRY
        .lock()
        .map_err(|e| anyhow!("failed to lock exporter registry: {e}"))?;
    let Some(old_exporter) = ht.get(name) else {
        return Err(anyhow!("no exporter with name {name} found"));
    };

    let config = config.unwrap_or_else(|| old_exporter._clone_config());
    let exporter = old_exporter._reload(config)?;
    if let Some(old_exporter) = ht.insert(name.clone(), exporter) {
        old_exporter._clean_to_offline();
    }
    Ok(())
}

pub(crate) fn get_or_insert_default(name: &NodeName) -> ArcExporter {
    let mut ht = RUNTIME_EXPORTER_REGISTRY.lock().unwrap();
    ht.entry(name.clone())
        .or_insert_with(|| super::discard::DiscardExporter::prepare_default(name))
        .clone()
}
