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
use once_cell::sync::Lazy;
use tokio::sync::broadcast;

use super::{ArcServer, ServerReloadCommand};
use crate::config::server::AnyServerConfig;
use crate::serve::dummy_close::DummyCloseServer;

static RUNTIME_SERVER_REGISTRY: Lazy<Mutex<HashMap<String, ArcServer>>> =
    Lazy::new(|| Mutex::new(HashMap::new()));
static OFFLINE_SERVER_SET: Lazy<Mutex<Vec<ArcServer>>> = Lazy::new(|| Mutex::new(Vec::new()));

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

pub(super) fn add(name: String, server: ArcServer) -> anyhow::Result<()> {
    let mut ht = RUNTIME_SERVER_REGISTRY.lock().unwrap();
    server._start_runtime(&server)?;
    if let Some(old_server) = ht.insert(name, server) {
        old_server._abort_runtime();
        add_offline(old_server);
    }
    Ok(())
}

pub(super) fn del(name: &str) {
    let mut ht = RUNTIME_SERVER_REGISTRY.lock().unwrap();
    if let Some(old_server) = ht.remove(name) {
        old_server._abort_runtime();
        add_offline(old_server);
    }
}

pub(crate) fn get_names() -> HashSet<String> {
    let mut names = HashSet::new();
    let ht = RUNTIME_SERVER_REGISTRY.lock().unwrap();
    for name in ht.keys() {
        names.insert(name.to_string());
    }
    names
}

pub(super) fn get_config(name: &str) -> Option<AnyServerConfig> {
    let ht = RUNTIME_SERVER_REGISTRY.lock().unwrap();
    ht.get(name).map(|server| server._clone_config())
}

pub(crate) fn get_server(name: &str) -> Option<ArcServer> {
    let ht = RUNTIME_SERVER_REGISTRY.lock().unwrap();
    ht.get(name).map(Arc::clone)
}

pub(super) fn update_config_in_place(
    name: &str,
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

pub(super) fn reload_only_config(name: &str, config: AnyServerConfig) -> anyhow::Result<()> {
    let mut ht = RUNTIME_SERVER_REGISTRY.lock().unwrap();
    let old_server = match ht.get(name) {
        Some(server) => server,
        None => return Err(anyhow!("no server with name {name} found")),
    };

    let server = old_server._reload_with_old_notifier(config)?;
    if let Some(old_server) = ht.insert(name.to_string(), Arc::clone(&server)) {
        // do not abort the runtime, as it's reused
        add_offline(old_server);
    }
    server._reload_config_notify_runtime();
    Ok(())
}

pub(super) fn reload_and_respawn(name: &str, config: AnyServerConfig) -> anyhow::Result<()> {
    let mut ht = RUNTIME_SERVER_REGISTRY.lock().unwrap();
    let old_server = match ht.get(name) {
        Some(server) => server,
        None => return Err(anyhow!("no server with name {name} found")),
    };

    let server = old_server._reload_with_new_notifier(config)?;
    server._start_runtime(&server)?;
    if let Some(old_server) = ht.insert(name.to_string(), server) {
        old_server._abort_runtime();
        add_offline(old_server);
    }
    Ok(())
}

pub(crate) fn foreach_online<F>(mut f: F)
where
    F: FnMut(&str, &ArcServer),
{
    let ht = RUNTIME_SERVER_REGISTRY.lock().unwrap();
    for (name, server) in ht.iter() {
        f(name, server)
    }
}

/// the notifier should be got while holding the lock
pub(crate) fn get_with_notifier(
    name: &str,
) -> (ArcServer, broadcast::Receiver<ServerReloadCommand>) {
    let mut ht = RUNTIME_SERVER_REGISTRY.lock().unwrap();
    let server = ht
        .entry(name.to_string())
        .or_insert_with(|| DummyCloseServer::prepare_default(name));
    let server_reload_channel = server._get_reload_notifier();
    (Arc::clone(server), server_reload_channel)
}
