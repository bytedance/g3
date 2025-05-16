/*
 * SPDX-License-Identifier: Apache-2.0
 * Copyright 2025 ByteDance and/or its affiliates.
 */

use std::collections::{HashMap, HashSet};
use std::sync::Mutex;

use anyhow::anyhow;
use foldhash::fast::FixedState;

use g3_types::metrics::NodeName;

use super::{ArcCollector, ArcCollectorInternal};
use crate::config::collector::AnyCollectorConfig;

static RUNTIME_COLLECTOR_REGISTRY: Mutex<CollectorRegistry> = Mutex::new(CollectorRegistry::new());

pub(super) struct CollectorRegistry {
    inner: HashMap<NodeName, ArcCollectorInternal, FixedState>,
}

impl CollectorRegistry {
    const fn new() -> Self {
        CollectorRegistry {
            inner: HashMap::with_hasher(FixedState::with_seed(0)),
        }
    }

    fn add(&mut self, name: NodeName, collector: ArcCollectorInternal) {
        if let Some(old_collector) = self.inner.insert(name, collector) {
            old_collector._clean_to_offline();
        }
    }

    fn del(&mut self, name: &NodeName) {
        if let Some(old_collector) = self.inner.remove(name) {
            old_collector._clean_to_offline();
        }
    }

    fn foreach<F>(&self, mut f: F)
    where
        F: FnMut(&NodeName, &ArcCollectorInternal),
    {
        for (name, collector) in self.inner.iter() {
            f(name, collector);
        }
    }

    fn get_names(&self) -> HashSet<NodeName> {
        self.inner.keys().cloned().collect()
    }

    fn get_config(&self, name: &NodeName) -> Option<AnyCollectorConfig> {
        self.inner.get(name).map(|collect| collect._clone_config())
    }

    fn get_collector(&self, name: &NodeName) -> Option<ArcCollectorInternal> {
        self.inner.get(name).cloned()
    }

    pub(super) fn reload(
        &mut self,
        name: &NodeName,
        config: Option<AnyCollectorConfig>,
    ) -> anyhow::Result<()> {
        let Some(old_collector) = self.inner.get(name) else {
            return Err(anyhow!("no collector with name {name} found"));
        };
        let old_collector = old_collector.clone();
        let config = config.unwrap_or_else(|| old_collector._clone_config());
        let collector = old_collector._reload(config, self)?;
        self.add(name.clone(), collector);
        Ok(())
    }

    pub(super) fn get_or_insert_default(&mut self, name: &NodeName) -> ArcCollector {
        self.inner
            .entry(name.clone())
            .or_insert_with(|| super::discard::DiscardCollector::prepare_default(name))
            .clone()
    }
}

pub(super) fn add(collector: ArcCollectorInternal) {
    let name = collector.name().clone();
    let mut r = RUNTIME_COLLECTOR_REGISTRY.lock().unwrap();
    r.add(name, collector)
}

pub(super) fn del(name: &NodeName) {
    let mut r = RUNTIME_COLLECTOR_REGISTRY.lock().unwrap();
    r.del(name);
}

pub(super) fn foreach<F>(f: F)
where
    F: FnMut(&NodeName, &ArcCollectorInternal),
{
    let r = RUNTIME_COLLECTOR_REGISTRY.lock().unwrap();
    r.foreach(f);
}

pub(crate) fn get_names() -> HashSet<NodeName> {
    let r = RUNTIME_COLLECTOR_REGISTRY.lock().unwrap();
    r.get_names()
}

pub(super) fn get_config(name: &NodeName) -> Option<AnyCollectorConfig> {
    let r = RUNTIME_COLLECTOR_REGISTRY.lock().unwrap();
    r.get_config(name)
}

fn get_collector(name: &NodeName) -> Option<ArcCollectorInternal> {
    let r = RUNTIME_COLLECTOR_REGISTRY.lock().unwrap();
    r.get_collector(name)
}

pub(super) fn update_config(name: &NodeName, config: AnyCollectorConfig) -> anyhow::Result<()> {
    let Some(collector) = get_collector(name) else {
        return Err(anyhow!("no collector with name {name} found"));
    };
    collector._update_config(config)
}

pub(super) fn reload_existed(
    name: &NodeName,
    config: Option<AnyCollectorConfig>,
) -> anyhow::Result<()> {
    let mut r = RUNTIME_COLLECTOR_REGISTRY
        .lock()
        .map_err(|e| anyhow!("failed to lock collector registry: {e}"))?;
    r.reload(name, config)
}

pub(crate) fn get_or_insert_default(name: &NodeName) -> ArcCollector {
    let mut r = RUNTIME_COLLECTOR_REGISTRY.lock().unwrap();
    r.get_or_insert_default(name)
}
