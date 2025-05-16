/*
 * SPDX-License-Identifier: Apache-2.0
 * Copyright 2025 ByteDance and/or its affiliates.
 */

use std::collections::HashMap;
use std::sync::{Arc, Mutex};

use foldhash::fast::FixedState;

use g3_types::metrics::NodeName;

use super::AnyExporterConfig;

static INITIAL_EXPORTER_CONFIG_REGISTRY: Mutex<
    HashMap<NodeName, Arc<AnyExporterConfig>, FixedState>,
> = Mutex::new(HashMap::with_hasher(FixedState::with_seed(0)));

pub(crate) fn clear() {
    let mut ht = INITIAL_EXPORTER_CONFIG_REGISTRY.lock().unwrap();
    ht.clear();
}

pub(super) fn add(exporter: AnyExporterConfig) -> Option<AnyExporterConfig> {
    let name = exporter.name().clone();
    let collector = Arc::new(exporter);
    let mut ht = INITIAL_EXPORTER_CONFIG_REGISTRY.lock().unwrap();
    ht.insert(name, collector).map(|v| v.as_ref().clone())
}

pub(crate) fn get_all() -> Vec<Arc<AnyExporterConfig>> {
    let ht = INITIAL_EXPORTER_CONFIG_REGISTRY.lock().unwrap();
    ht.values().cloned().collect()
}
