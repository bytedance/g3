/*
 * SPDX-License-Identifier: Apache-2.0
 * Copyright 2025 ByteDance and/or its affiliates.
 */

use std::collections::HashMap;
use std::sync::{Arc, Mutex};

use foldhash::fast::FixedState;

use g3_types::metrics::NodeName;

use super::AnyImporterConfig;

static INITIAL_IMPORTER_CONFIG_REGISTRY: Mutex<
    HashMap<NodeName, Arc<AnyImporterConfig>, FixedState>,
> = Mutex::new(HashMap::with_hasher(FixedState::with_seed(0)));

pub(crate) fn clear() {
    let mut ht = INITIAL_IMPORTER_CONFIG_REGISTRY.lock().unwrap();
    ht.clear();
}

pub(super) fn add(importer: AnyImporterConfig) -> Option<AnyImporterConfig> {
    let name = importer.name().clone();
    let importer = Arc::new(importer);
    let mut ht = INITIAL_IMPORTER_CONFIG_REGISTRY.lock().unwrap();
    ht.insert(name, importer).map(|v| v.as_ref().clone())
}

pub(crate) fn get_all() -> Vec<Arc<AnyImporterConfig>> {
    let ht = INITIAL_IMPORTER_CONFIG_REGISTRY.lock().unwrap();
    ht.values().cloned().collect()
}
