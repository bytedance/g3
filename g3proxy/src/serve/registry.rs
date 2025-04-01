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

use anyhow::anyhow;
use foldhash::fast::FixedState;

use g3_types::metrics::NodeName;

use super::ArcServer;
use crate::config::server::AnyServerConfig;
use crate::serve::dummy_close::DummyCloseServer;

static RUNTIME_SERVER_REGISTRY: Mutex<HashMap<NodeName, ArcServer, FixedState>> =
    Mutex::new(HashMap::with_hasher(FixedState::with_seed(0)));
static OFFLINE_SERVER_SET: Mutex<Vec<ArcServer>> = Mutex::new(Vec::new());

pub(super) fn add_offline(old_server: ArcServer) {
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
    F: FnMut(&ArcServer),
{
    let set = OFFLINE_SERVER_SET.lock().unwrap();
    for server in set.iter() {
        f(server)
    }
}

pub(super) fn add(name: NodeName, server: ArcServer) -> anyhow::Result<()> {
    let mut ht = RUNTIME_SERVER_REGISTRY.lock().unwrap();
    server._start_runtime(&server)?;
    if let Some(old_server) = ht.insert(name, server) {
        old_server._abort_runtime();
        add_offline(old_server);
    }
    Ok(())
}

pub(super) fn del(name: &NodeName) {
    let mut ht = RUNTIME_SERVER_REGISTRY.lock().unwrap();
    if let Some(old_server) = ht.remove(name) {
        old_server._abort_runtime();
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

pub(super) fn get_config(name: &NodeName) -> Option<AnyServerConfig> {
    let ht = RUNTIME_SERVER_REGISTRY.lock().unwrap();
    ht.get(name).map(|server| server._clone_config())
}

pub(crate) fn get_server(name: &NodeName) -> Option<ArcServer> {
    let ht = RUNTIME_SERVER_REGISTRY.lock().unwrap();
    ht.get(name).cloned()
}

pub(super) fn update_config_in_place(
    name: &NodeName,
    flags: u64,
    config: AnyServerConfig,
) -> anyhow::Result<()> {
    let ht = RUNTIME_SERVER_REGISTRY.lock().unwrap();
    if let Some(server) = ht.get(name) {
        server._update_config_in_place(flags, config)
    } else {
        Err(anyhow!("no server with name {name} found"))
    }
}

pub(super) fn reload_no_respawn(name: &NodeName, config: AnyServerConfig) -> anyhow::Result<()> {
    let mut ht = RUNTIME_SERVER_REGISTRY.lock().unwrap();
    let Some(old_server) = ht.get(name) else {
        return Err(anyhow!("no server with name {name} found"));
    };

    let server = old_server._reload_with_old_notifier(config)?;
    if let Some(old_server) = ht.insert(name.clone(), Arc::clone(&server)) {
        // do not abort the runtime, as it's reused
        add_offline(old_server);
    }
    server._reload_config_notify_runtime();
    Ok(())
}

pub(crate) fn reload_only_escaper(name: &NodeName) -> anyhow::Result<()> {
    let ht = RUNTIME_SERVER_REGISTRY.lock().unwrap();
    let Some(server) = ht.get(name) else {
        return Err(anyhow!("no server with name {name} found"));
    };

    server._update_escaper_in_place();
    Ok(())
}

pub(crate) fn reload_only_user_group(name: &NodeName) -> anyhow::Result<()> {
    let ht = RUNTIME_SERVER_REGISTRY.lock().unwrap();
    let Some(server) = ht.get(name) else {
        return Err(anyhow!("no server with name {name} found"));
    };

    server._update_user_group_in_place();
    Ok(())
}

pub(crate) fn reload_only_auditor(name: &NodeName) -> anyhow::Result<()> {
    let ht = RUNTIME_SERVER_REGISTRY.lock().unwrap();
    let Some(server) = ht.get(name) else {
        return Err(anyhow!("no server with name {name} found"));
    };

    server._update_audit_handle_in_place()?;
    Ok(())
}

pub(super) fn reload_and_respawn(name: &NodeName, config: AnyServerConfig) -> anyhow::Result<()> {
    let mut ht = RUNTIME_SERVER_REGISTRY.lock().unwrap();
    let Some(old_server) = ht.get(name) else {
        return Err(anyhow!("no server with name {name} found"));
    };

    let server = old_server._reload_with_new_notifier(config)?;
    server._start_runtime(&server)?;
    if let Some(old_server) = ht.insert(name.clone(), server) {
        old_server._abort_runtime();
        add_offline(old_server);
    }
    Ok(())
}

pub(crate) fn foreach_online<F>(mut f: F)
where
    F: FnMut(&NodeName, &ArcServer),
{
    let ht = RUNTIME_SERVER_REGISTRY.lock().unwrap();
    for (name, server) in ht.iter() {
        f(name, server)
    }
}

pub(crate) fn get_or_insert_default(name: &NodeName) -> ArcServer {
    let mut ht = RUNTIME_SERVER_REGISTRY.lock().unwrap();
    ht.entry(name.clone())
        .or_insert_with(|| DummyCloseServer::prepare_default(name))
        .clone()
}
