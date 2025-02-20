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
use std::sync::{Arc, Mutex};

use anyhow::{Context, anyhow};
use foldhash::fast::FixedState;

use g3_types::metrics::NodeName;

use super::KeyServer;
use crate::config::server::KeyServerConfig;

static RUNTIME_SERVER_REGISTRY: Mutex<HashMap<NodeName, Arc<KeyServer>, FixedState>> =
    Mutex::new(HashMap::with_hasher(FixedState::with_seed(0)));
static OFFLINE_SERVER_SET: Mutex<Vec<Arc<KeyServer>>> = Mutex::new(Vec::new());

pub(super) fn add_offline(old_server: Arc<KeyServer>) {
    let mut set = OFFLINE_SERVER_SET.lock().unwrap();
    set.push(old_server);
}

pub(super) fn retain_offline() {
    let mut set = OFFLINE_SERVER_SET.lock().unwrap();
    set.retain(|server| {
        if server.alive_count() == 0 {
            Arc::strong_count(server) > 1
        } else {
            let quit_policy = server.quit_policy().clone();
            if !quit_policy.force_quit_scheduled() {
                quit_policy.set_force_quit_scheduled();
                tokio::spawn(async move {
                    let wait_time = g3_daemon::runtime::config::get_task_wait_timeout();
                    tokio::time::sleep(wait_time).await;
                    quit_policy.set_force_quit();
                });
            }
            true
        }
    });
}

pub(super) fn foreach_offline<F>(mut f: F)
where
    F: FnMut(&Arc<KeyServer>),
{
    let set = OFFLINE_SERVER_SET.lock().unwrap();
    for server in set.iter() {
        f(server)
    }
}

pub(super) fn add(name: NodeName, server: Arc<KeyServer>) -> anyhow::Result<()> {
    let mut ht = RUNTIME_SERVER_REGISTRY.lock().unwrap();
    server.start_runtime(&server)?;
    if let Some(old_server) = ht.insert(name, server) {
        old_server.abort_runtime();
        add_offline(old_server);
    }
    Ok(())
}

pub(super) fn add_lazy(name: NodeName, server: Arc<KeyServer>) {
    let mut ht = RUNTIME_SERVER_REGISTRY.lock().unwrap();
    // no start runtime
    if let Some(_old_server) = ht.insert(name, server) {
        // no offline
    }
}

pub(super) fn del(name: &NodeName) {
    let mut ht = RUNTIME_SERVER_REGISTRY.lock().unwrap();
    if let Some(old_server) = ht.remove(name) {
        old_server.abort_runtime();
        add_offline(old_server);
    }
}

pub(crate) fn get_names() -> HashSet<NodeName> {
    let mut names = HashSet::new();
    let ht = RUNTIME_SERVER_REGISTRY.lock().unwrap();
    for name in ht.keys() {
        names.insert(name.clone());
    }
    names
}

pub(super) fn get_config(name: &NodeName) -> Option<Arc<KeyServerConfig>> {
    let ht = RUNTIME_SERVER_REGISTRY.lock().unwrap();
    ht.get(name).map(|server| server.clone_config())
}

pub(crate) fn get_server(name: &NodeName) -> Option<Arc<KeyServer>> {
    let ht = RUNTIME_SERVER_REGISTRY.lock().unwrap();
    ht.get(name).cloned()
}

pub(super) fn reload_and_respawn(name: &NodeName, config: KeyServerConfig) -> anyhow::Result<()> {
    let mut ht = RUNTIME_SERVER_REGISTRY.lock().unwrap();
    let old_server = match ht.get(name) {
        Some(server) => server,
        None => return Err(anyhow!("no server with name {name} found")),
    };

    let server = Arc::new(old_server.reload_with_new_notifier(config));
    server.start_runtime(&server)?;
    if let Some(old_server) = ht.insert(name.clone(), server) {
        old_server.abort_runtime();
        add_offline(old_server);
    }
    Ok(())
}

pub(crate) fn foreach_online<F>(mut f: F)
where
    F: FnMut(&NodeName, &Arc<KeyServer>),
{
    let ht = RUNTIME_SERVER_REGISTRY.lock().unwrap();
    for (name, server) in ht.iter() {
        f(name, server)
    }
}

pub(crate) fn foreach_start_runtime() -> anyhow::Result<()> {
    let ht = RUNTIME_SERVER_REGISTRY.lock().unwrap();
    for (name, server) in ht.iter() {
        server
            .start_runtime(server)
            .context(format!("failed to start runtime for {name}"))?;
    }
    Ok(())
}
