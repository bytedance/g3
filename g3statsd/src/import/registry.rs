/*
 * SPDX-License-Identifier: Apache-2.0
 * Copyright 2025 ByteDance and/or its affiliates.
 */

use std::collections::{HashMap, HashSet};
use std::sync::{Arc, Mutex};

use anyhow::anyhow;
use foldhash::fast::FixedState;

use g3_types::metrics::NodeName;

use super::{ArcImporter, ArcImporterInternal};
use crate::config::importer::AnyImporterConfig;

static RUNTIME_IMPORTER_REGISTRY: Mutex<ImporterRegistry> = Mutex::new(ImporterRegistry::new());

pub(super) struct ImporterRegistry {
    inner: HashMap<NodeName, ArcImporterInternal, FixedState>,
}

impl ImporterRegistry {
    const fn new() -> Self {
        ImporterRegistry {
            inner: HashMap::with_hasher(FixedState::with_seed(0)),
        }
    }

    fn add(&mut self, name: NodeName, importer: ArcImporterInternal) -> anyhow::Result<()> {
        importer._start_runtime(importer.clone())?;
        if let Some(old_importer) = self.inner.insert(name, importer) {
            old_importer._abort_runtime();
        }
        Ok(())
    }

    fn del(&mut self, name: &NodeName) {
        if let Some(old_importer) = self.inner.remove(name) {
            old_importer._abort_runtime();
        }
    }

    fn get_names(&self) -> HashSet<NodeName> {
        self.inner.keys().cloned().collect()
    }

    fn get_config(&self, name: &NodeName) -> Option<AnyImporterConfig> {
        self.inner
            .get(name)
            .map(|importer| importer._clone_config())
    }

    fn get_importer(&self, name: &NodeName) -> Option<ArcImporterInternal> {
        self.inner.get(name).cloned()
    }

    fn reload_no_respawn(
        &mut self,
        name: &NodeName,
        config: AnyImporterConfig,
    ) -> anyhow::Result<()> {
        let Some(old_importer) = self.inner.get(name) else {
            return Err(anyhow!("no importer with name {name} found"));
        };

        let old_importer = old_importer.clone();
        let importer = old_importer._reload_with_old_notifier(config, self)?;
        if let Some(_old_importer) = self.inner.insert(name.clone(), Arc::clone(&importer)) {
            // do not abort the runtime, as it's reused
        }
        importer._reload_config_notify_runtime();
        Ok(())
    }

    fn reload_and_respawn(
        &mut self,
        name: &NodeName,
        config: AnyImporterConfig,
    ) -> anyhow::Result<()> {
        let Some(old_importer) = self.inner.get(name) else {
            return Err(anyhow!("no importer with name {name} found"));
        };

        let old_importer = old_importer.clone();
        let importer = old_importer._reload_with_new_notifier(config, self)?;
        self.add(name.clone(), importer)
    }

    fn foreach<F>(&self, mut f: F)
    where
        F: FnMut(&NodeName, &ArcImporterInternal),
    {
        for (name, importer) in self.inner.iter() {
            f(name, importer)
        }
    }

    pub(super) fn get_or_insert_default(&mut self, name: &NodeName) -> ArcImporter {
        self.inner
            .entry(name.clone())
            .or_insert_with(|| super::dummy::DummyImporter::prepare_default(name))
            .clone()
    }
}

pub(super) fn add(importer: ArcImporterInternal) -> anyhow::Result<()> {
    let name = importer.name().clone();
    let mut r = RUNTIME_IMPORTER_REGISTRY
        .lock()
        .map_err(|e| anyhow!("failed to lock importer registry: {e}"))?;
    r.add(name, importer)
}

pub(super) fn del(name: &NodeName) {
    let mut r = RUNTIME_IMPORTER_REGISTRY.lock().unwrap();
    r.del(name);
}

pub(crate) fn get_names() -> HashSet<NodeName> {
    let r = RUNTIME_IMPORTER_REGISTRY.lock().unwrap();
    r.get_names()
}

pub(super) fn get_config(name: &NodeName) -> Option<AnyImporterConfig> {
    let r = RUNTIME_IMPORTER_REGISTRY.lock().unwrap();
    r.get_config(name)
}

pub(super) fn reload_no_respawn(name: &NodeName, config: AnyImporterConfig) -> anyhow::Result<()> {
    let mut r = RUNTIME_IMPORTER_REGISTRY
        .lock()
        .map_err(|e| anyhow!("failed to lock importer registry: {e}"))?;
    r.reload_no_respawn(name, config)
}

fn get_importer(name: &NodeName) -> Option<ArcImporterInternal> {
    let r = RUNTIME_IMPORTER_REGISTRY.lock().unwrap();
    r.get_importer(name)
}

pub(super) fn reload_only_collector(name: &NodeName) -> anyhow::Result<()> {
    let Some(importer) = get_importer(name) else {
        return Err(anyhow!("no importer with name {name} found"));
    };
    importer._update_collector_in_place();
    Ok(())
}

pub(super) fn reload_and_respawn(name: &NodeName, config: AnyImporterConfig) -> anyhow::Result<()> {
    let mut r = RUNTIME_IMPORTER_REGISTRY
        .lock()
        .map_err(|e| anyhow!("failed to lock importer registry: {e}"))?;
    r.reload_and_respawn(name, config)
}

pub(super) fn foreach<F>(f: F)
where
    F: FnMut(&NodeName, &ArcImporterInternal),
{
    let r = RUNTIME_IMPORTER_REGISTRY.lock().unwrap();
    r.foreach(f)
}

pub(crate) fn get_or_insert_default(name: &NodeName) -> ArcImporter {
    let mut r = RUNTIME_IMPORTER_REGISTRY.lock().unwrap();
    r.get_or_insert_default(name)
}
