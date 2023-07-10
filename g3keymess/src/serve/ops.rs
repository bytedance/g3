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

use std::collections::HashSet;
use std::sync::Arc;
use std::time::Duration;

use log::debug;
use once_cell::sync::Lazy;
use tokio::sync::Mutex;

use g3_types::metrics::MetricsName;

use super::{registry, KeyServer};
use crate::config::server::KeyServerConfig;

static SERVER_OPS_LOCK: Lazy<Mutex<()>> = Lazy::new(|| Mutex::new(()));

pub fn spawn_offline_clean() {
    tokio::spawn(async {
        let mut interval = tokio::time::interval(Duration::from_secs(60));
        interval.tick().await;
        loop {
            registry::retain_offline();
            interval.tick().await;
        }
    });
}

pub async fn spawn_all() -> anyhow::Result<()> {
    let _guard = SERVER_OPS_LOCK.lock().await;

    let mut new_names = HashSet::<MetricsName>::new();

    let all_config = crate::config::server::get_all();
    for config in all_config {
        let name = config.name();
        new_names.insert(name.clone());
        match registry::get_config(name) {
            Some(old) => {
                debug!("reloading server {name}");
                reload_old_unlocked(old.as_ref(), config.as_ref().clone())?;
                debug!("server {name} reload OK");
            }
            None => {
                debug!("creating server {name}");
                spawn_new_unlocked(config.as_ref().clone())?;
                debug!("server {name} create OK");
            }
        }
    }

    for name in &registry::get_names() {
        if !new_names.contains(name) {
            debug!("deleting server {name}");
            registry::del(name);
            debug!("server {name} deleted");
        }
    }

    Ok(())
}

pub async fn stop_all() {
    let _guard = SERVER_OPS_LOCK.lock().await;

    registry::foreach_online(|_name, server| {
        server.abort_runtime();
        registry::add_offline(Arc::clone(server));
    });
}

fn reload_old_unlocked(old: &KeyServerConfig, new: KeyServerConfig) -> anyhow::Result<()> {
    let name = old.name();
    debug!("server {name} reload: will respawn with old stats");
    registry::reload_and_respawn(name, new)
}

// use async fn to allow tokio schedule
fn spawn_new_unlocked(config: KeyServerConfig) -> anyhow::Result<()> {
    let name = config.name().clone();
    let server = KeyServer::prepare_initial(config);
    registry::add(name, Arc::new(server))?;
    Ok(())
}

pub(crate) async fn wait_all_tasks<F>(wait_timeout: Duration, quit_timeout: Duration, on_timeout: F)
where
    F: Fn(&MetricsName, i32),
{
    let loop_wait = async {
        loop {
            let mut has_pending = false;

            registry::foreach_offline(|server| {
                if server.alive_count() > 0 {
                    has_pending = true;
                }
            });

            if !has_pending {
                if let Some(stat_config) = g3_daemon::stat::config::get_global_stat_config() {
                    // sleep more time for flushing metrics
                    tokio::time::sleep(stat_config.emit_duration * 2).await;
                }
                break;
            }

            tokio::time::sleep(Duration::from_secs(4)).await;
        }
    };

    tokio::pin!(loop_wait);

    debug!("will wait {wait_timeout:?} for all tasks to be finished");
    if tokio::time::timeout(wait_timeout, &mut loop_wait)
        .await
        .is_ok()
    {
        return;
    }

    // enable force_quit and wait more time
    force_quit_offline_servers();

    debug!("will wait {quit_timeout:?} for all tasks to force quit");
    if tokio::time::timeout(quit_timeout, &mut loop_wait)
        .await
        .is_err()
    {
        registry::foreach_offline(|server| {
            on_timeout(server.name(), server.alive_count());
        });
    }
}

pub(crate) fn force_quit_offline_servers() {
    registry::foreach_offline(|server| {
        server.quit_policy().set_force_quit();
    });
}
