/*
 * SPDX-License-Identifier: Apache-2.0
 * Copyright 2025 ByteDance and/or its affiliates.
 */

use std::collections::{HashMap, HashSet};
use std::sync::Mutex;

use anyhow::anyhow;
use foldhash::fast::FixedState;

use g3_types::metrics::NodeName;

use super::{ArcExporter, ArcExporterInternal};
use crate::config::exporter::AnyExporterConfig;

static RUNTIME_EXPORTER_REGISTRY: Mutex<HashMap<NodeName, ArcExporterInternal, FixedState>> =
    Mutex::new(HashMap::with_hasher(FixedState::with_seed(0)));

pub(super) fn add(exporter: ArcExporterInternal) {
    let name = exporter.name().clone();
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
