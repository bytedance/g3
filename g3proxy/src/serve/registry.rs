/*
 * SPDX-License-Identifier: Apache-2.0
 * Copyright 2023-2025 ByteDance and/or its affiliates.
 */

use std::collections::{HashMap, HashSet};
use std::sync::{Arc, Mutex};

use anyhow::anyhow;
use foldhash::fast::FixedState;

use g3_types::metrics::NodeName;

use super::{ArcServer, ArcServerInternal};
use crate::config::server::AnyServerConfig;
use crate::serve::dummy_close::DummyCloseServer;

static RUNTIME_SERVER_REGISTRY: Mutex<ServerRegistry> = Mutex::new(ServerRegistry::new());
static OFFLINE_SERVER_SET: Mutex<Vec<ArcServerInternal>> = Mutex::new(Vec::new());

pub(super) struct ServerRegistry {
    inner: HashMap<NodeName, ArcServerInternal, FixedState>,
}

impl ServerRegistry {
    const fn new() -> Self {
        ServerRegistry {
            inner: HashMap::with_hasher(FixedState::with_seed(0)),
        }
    }

    fn add(&mut self, name: NodeName, server: ArcServerInternal) -> anyhow::Result<()> {
        server._start_runtime(server.clone())?;
        if let Some(old_server) = self.inner.insert(name, server) {
            old_server._abort_runtime();
            add_offline(old_server);
        }
        Ok(())
    }

    fn del(&mut self, name: &NodeName) {
        if let Some(old_server) = self.inner.remove(name) {
            old_server._abort_runtime();
            add_offline(old_server);
        }
    }

    fn get_names(&self) -> HashSet<NodeName> {
        self.inner.keys().cloned().collect()
    }

    fn get_config(&self, name: &NodeName) -> Option<AnyServerConfig> {
        self.inner.get(name).map(|server| server._clone_config())
    }

    fn get_server(&self, name: &NodeName) -> Option<ArcServerInternal> {
        self.inner.get(name).cloned()
    }

    fn reload_no_respawn(
        &mut self,
        name: &NodeName,
        config: AnyServerConfig,
    ) -> anyhow::Result<()> {
        let Some(old_server) = self.inner.get(name) else {
            return Err(anyhow!("no server with name {name} found"));
        };

        let old_server = old_server.clone();
        let server = old_server._reload_with_old_notifier(config, self)?;
        if let Some(old_server) = self.inner.insert(name.clone(), Arc::clone(&server)) {
            // do not abort the runtime, as it's reused
            add_offline(old_server);
        }
        server._reload_config_notify_runtime();
        Ok(())
    }

    fn reload_and_respawn(
        &mut self,
        name: &NodeName,
        config: AnyServerConfig,
    ) -> anyhow::Result<()> {
        let Some(old_server) = self.inner.get(name) else {
            return Err(anyhow!("no server with name {name} found"));
        };

        let old_server = old_server.clone();
        let server = old_server._reload_with_new_notifier(config, self)?;
        self.add(name.clone(), server)
    }

    fn foreach<F>(&self, mut f: F)
    where
        F: FnMut(&NodeName, &ArcServerInternal),
    {
        for (name, server) in self.inner.iter() {
            f(name, server)
        }
    }

    pub(super) fn get_or_insert_default(&mut self, name: &NodeName) -> ArcServer {
        self.inner
            .entry(name.clone())
            .or_insert_with(|| DummyCloseServer::prepare_default(name))
            .clone()
    }
}

pub(super) fn add_offline(old_server: ArcServerInternal) {
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
    F: FnMut(&ArcServerInternal),
{
    let set = OFFLINE_SERVER_SET.lock().unwrap();
    for server in set.iter() {
        f(server)
    }
}

pub(super) fn add(name: NodeName, server: ArcServerInternal) -> anyhow::Result<()> {
    let mut sr = RUNTIME_SERVER_REGISTRY
        .lock()
        .map_err(|e| anyhow!("failed to lock server registry: {e}"))?;
    sr.add(name, server)
}

pub(super) fn del(name: &NodeName) {
    let mut sr = RUNTIME_SERVER_REGISTRY.lock().unwrap();
    sr.del(name);
}

pub(crate) fn get_names() -> HashSet<NodeName> {
    let sr = RUNTIME_SERVER_REGISTRY.lock().unwrap();
    sr.get_names()
}

pub(super) fn get_config(name: &NodeName) -> Option<AnyServerConfig> {
    let sr = RUNTIME_SERVER_REGISTRY.lock().unwrap();
    sr.get_config(name)
}

pub(super) fn get_server(name: &NodeName) -> Option<ArcServerInternal> {
    let sr = RUNTIME_SERVER_REGISTRY.lock().unwrap();
    sr.get_server(name)
}

fn check_get_server(name: &NodeName) -> anyhow::Result<ArcServerInternal> {
    get_server(name).ok_or_else(|| anyhow!("no server with name {name} found"))
}

pub(super) fn update_config_in_place(
    name: &NodeName,
    flags: u64,
    config: AnyServerConfig,
) -> anyhow::Result<()> {
    let server = check_get_server(name)?;
    server._update_config_in_place(flags, config)
}

pub(super) fn reload_no_respawn(name: &NodeName, config: AnyServerConfig) -> anyhow::Result<()> {
    let mut sr = RUNTIME_SERVER_REGISTRY
        .lock()
        .map_err(|e| anyhow!("failed to lock server registry: {e}"))?;
    sr.reload_no_respawn(name, config)
}

pub(super) fn reload_only_escaper(name: &NodeName) -> anyhow::Result<()> {
    let server = check_get_server(name)?;
    server._update_escaper_in_place();
    Ok(())
}

pub(super) fn reload_only_user_group(name: &NodeName) -> anyhow::Result<()> {
    let server = check_get_server(name)?;
    server._update_user_group_in_place();
    Ok(())
}

pub(super) fn reload_only_auditor(name: &NodeName) -> anyhow::Result<()> {
    let server = check_get_server(name)?;
    server._update_audit_handle_in_place()
}

pub(super) fn reload_and_respawn(name: &NodeName, config: AnyServerConfig) -> anyhow::Result<()> {
    let mut sr = RUNTIME_SERVER_REGISTRY
        .lock()
        .map_err(|e| anyhow!("failed to lock server registry: {e}"))?;
    sr.reload_and_respawn(name, config)
}

pub(crate) fn foreach_online<F>(f: F)
where
    F: FnMut(&NodeName, &ArcServerInternal),
{
    let sr = RUNTIME_SERVER_REGISTRY.lock().unwrap();
    sr.foreach(f)
}

pub(crate) fn get_or_insert_default(name: &NodeName) -> ArcServer {
    let mut sr = RUNTIME_SERVER_REGISTRY.lock().unwrap();
    sr.get_or_insert_default(name)
}
