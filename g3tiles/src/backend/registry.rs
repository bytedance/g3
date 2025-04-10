/*
 * Copyright 2024 ByteDance and/or its affiliates.
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

use super::dummy_close::DummyCloseBackend;
use super::{ArcBackend, ArcBackendInternal};
use crate::config::backend::AnyBackendConfig;

static RUNTIME_BACKEND_REGISTRY: Mutex<BackendRegistry> = Mutex::new(BackendRegistry::new());

pub(super) struct BackendRegistry {
    inner: HashMap<NodeName, ArcBackendInternal, FixedState>,
}

impl BackendRegistry {
    const fn new() -> Self {
        BackendRegistry {
            inner: HashMap::with_hasher(FixedState::with_seed(0)),
        }
    }

    fn add(&mut self, name: NodeName, connector: ArcBackendInternal) {
        if let Some(_old) = self.inner.insert(name, connector) {}
    }

    fn get(&self, name: &NodeName) -> Option<ArcBackendInternal> {
        self.inner.get(name).cloned()
    }

    fn del(&mut self, name: &NodeName) {
        if let Some(_old) = self.inner.remove(name) {}
    }

    fn foreach<F>(&self, mut f: F)
    where
        F: FnMut(&NodeName, &ArcBackendInternal),
    {
        for (name, backend) in self.inner.iter() {
            f(name, backend);
        }
    }

    fn get_names(&self) -> HashSet<NodeName> {
        self.inner.keys().cloned().collect()
    }

    fn get_config(&self, name: &NodeName) -> Option<AnyBackendConfig> {
        self.inner.get(name).map(|g| g._clone_config())
    }

    pub(super) fn reload(
        &mut self,
        name: &NodeName,
        config: Option<AnyBackendConfig>,
    ) -> anyhow::Result<()> {
        let Some(old_backend) = self.inner.get(name) else {
            return Err(anyhow!("no backend with name {name} found"));
        };

        let old_backend = old_backend.clone();
        let config = config.unwrap_or_else(|| old_backend._clone_config());
        let backend = old_backend._reload(config, self)?;
        self.add(name.clone(), backend);
        Ok(())
    }

    pub(super) fn get_or_insert_default(&mut self, name: &NodeName) -> ArcBackend {
        self.inner
            .entry(name.clone())
            .or_insert_with(|| DummyCloseBackend::prepare_default(name))
            .clone()
    }
}

pub(super) fn add(name: NodeName, connector: ArcBackendInternal) {
    let mut r = RUNTIME_BACKEND_REGISTRY.lock().unwrap();
    r.add(name, connector)
}

pub(super) fn get(name: &NodeName) -> Option<ArcBackendInternal> {
    let r = RUNTIME_BACKEND_REGISTRY.lock().unwrap();
    r.get(name)
}

pub(super) fn del(name: &NodeName) {
    let mut r = RUNTIME_BACKEND_REGISTRY.lock().unwrap();
    r.del(name);
}

pub(super) fn foreach<F>(f: F)
where
    F: FnMut(&NodeName, &ArcBackendInternal),
{
    let r = RUNTIME_BACKEND_REGISTRY.lock().unwrap();
    r.foreach(f);
}

pub(crate) fn get_names() -> HashSet<NodeName> {
    let r = RUNTIME_BACKEND_REGISTRY.lock().unwrap();
    r.get_names()
}

pub(super) fn get_config(name: &NodeName) -> Option<AnyBackendConfig> {
    let r = RUNTIME_BACKEND_REGISTRY.lock().unwrap();
    r.get_config(name)
}

pub(super) fn update_config_in_place(
    name: &NodeName,
    flags: u64,
    config: AnyBackendConfig,
) -> anyhow::Result<()> {
    let Some(backend) = get(name) else {
        return Err(anyhow!("no backend with name {name} found"));
    };
    backend._update_config_in_place(flags, config)
}

pub(super) fn reload_existed(
    name: &NodeName,
    config: Option<AnyBackendConfig>,
) -> anyhow::Result<()> {
    let mut r = RUNTIME_BACKEND_REGISTRY
        .lock()
        .map_err(|e| anyhow!("failed to lock backend registry: {e}"))?;
    r.reload(name, config)
}

pub(crate) fn get_or_insert_default(name: &NodeName) -> ArcBackend {
    let mut r = RUNTIME_BACKEND_REGISTRY.lock().unwrap();
    r.get_or_insert_default(name)
}
