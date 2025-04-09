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

use super::ArcCollector;
use crate::config::collector::AnyCollectorConfig;

static RUNTIME_COLLECTOR_REGISTRY: Mutex<CollectorRegistry> = Mutex::new(CollectorRegistry::new());

pub(crate) struct CollectorRegistry {
    inner: HashMap<NodeName, ArcCollector, FixedState>,
}

impl CollectorRegistry {
    const fn new() -> Self {
        CollectorRegistry {
            inner: HashMap::with_hasher(FixedState::with_seed(0)),
        }
    }

    fn add(&mut self, name: NodeName, collector: ArcCollector) {
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
        F: FnMut(&NodeName, &ArcCollector),
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

pub(super) fn add(name: NodeName, collector: ArcCollector) {
    let mut r = RUNTIME_COLLECTOR_REGISTRY.lock().unwrap();
    r.add(name, collector)
}

pub(super) fn del(name: &NodeName) {
    let mut r = RUNTIME_COLLECTOR_REGISTRY.lock().unwrap();
    r.del(name);
}

pub(crate) fn foreach<F>(f: F)
where
    F: FnMut(&NodeName, &ArcCollector),
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
