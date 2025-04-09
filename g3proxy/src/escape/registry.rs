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
use std::sync::Mutex;

use anyhow::anyhow;
use foldhash::fast::FixedState;

use g3_types::metrics::NodeName;

use super::ArcEscaper;
use super::dummy_deny::DummyDenyEscaper;
use crate::config::escaper::AnyEscaperConfig;

static RUNTIME_ESCAPER_REGISTRY: Mutex<EscaperRegistry> = Mutex::new(EscaperRegistry::new());

pub(crate) struct EscaperRegistry {
    inner: HashMap<NodeName, ArcEscaper, FixedState>,
}

impl EscaperRegistry {
    const fn new() -> Self {
        EscaperRegistry {
            inner: HashMap::with_hasher(FixedState::with_seed(0)),
        }
    }

    fn add(&mut self, name: NodeName, escaper: ArcEscaper) {
        if let Some(old_escaper) = self.inner.insert(name, escaper) {
            old_escaper._clean_to_offline();
        }
    }

    fn del(&mut self, name: &NodeName) {
        if let Some(old_escaper) = self.inner.remove(name) {
            old_escaper._clean_to_offline();
        }
    }

    fn foreach<F>(&self, mut f: F)
    where
        F: FnMut(&NodeName, &ArcEscaper),
    {
        for (name, escaper) in self.inner.iter() {
            f(name, escaper);
        }
    }

    fn get_names(&self) -> HashSet<NodeName> {
        self.inner.keys().cloned().collect()
    }

    fn get_escaper(&self, name: &NodeName) -> Option<ArcEscaper> {
        self.inner.get(name).cloned()
    }

    fn get_config(&self, name: &NodeName) -> Option<AnyEscaperConfig> {
        self.inner.get(name).map(|escaper| escaper._clone_config())
    }

    pub(super) fn reload(
        &mut self,
        name: &NodeName,
        config: Option<AnyEscaperConfig>,
    ) -> anyhow::Result<()> {
        let Some(old_escaper) = self.inner.get(name) else {
            return Err(anyhow!("no escaper with name {name} found"));
        };

        let old_escaper = old_escaper.clone();
        let config = config.unwrap_or_else(|| old_escaper._clone_config());
        let escaper = old_escaper._reload(config, self)?;
        self.add(name.clone(), escaper);
        Ok(())
    }

    pub(super) fn get_or_insert_default(&mut self, name: &NodeName) -> ArcEscaper {
        self.inner
            .entry(name.clone())
            .or_insert_with(|| DummyDenyEscaper::prepare_default(name))
            .clone()
    }
}

pub(super) fn add(name: NodeName, escaper: ArcEscaper) {
    let mut r = RUNTIME_ESCAPER_REGISTRY.lock().unwrap();
    r.add(name, escaper);
}

pub(super) fn del(name: &NodeName) {
    let mut r = RUNTIME_ESCAPER_REGISTRY.lock().unwrap();
    r.del(name);
}

pub(crate) fn foreach<F>(f: F)
where
    F: FnMut(&NodeName, &ArcEscaper),
{
    let r = RUNTIME_ESCAPER_REGISTRY.lock().unwrap();
    r.foreach(f);
}

pub(crate) fn get_names() -> HashSet<NodeName> {
    let r = RUNTIME_ESCAPER_REGISTRY.lock().unwrap();
    r.get_names()
}

pub(super) fn get_escaper(name: &NodeName) -> Option<ArcEscaper> {
    let r = RUNTIME_ESCAPER_REGISTRY.lock().unwrap();
    r.get_escaper(name)
}

pub(super) fn get_config(name: &NodeName) -> Option<AnyEscaperConfig> {
    let r = RUNTIME_ESCAPER_REGISTRY.lock().unwrap();
    r.get_config(name)
}

pub(super) fn reload_existed(
    name: &NodeName,
    config: Option<AnyEscaperConfig>,
) -> anyhow::Result<()> {
    let mut r = RUNTIME_ESCAPER_REGISTRY
        .lock()
        .map_err(|e| anyhow!("failed to lock escaper registry: {e}"))?;
    r.reload(name, config)
}

pub(crate) fn get_or_insert_default(name: &NodeName) -> ArcEscaper {
    let mut r = RUNTIME_ESCAPER_REGISTRY.lock().unwrap();
    r.get_or_insert_default(name)
}
