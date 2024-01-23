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

use std::collections::HashSet;

use anyhow::{anyhow, Context};
use log::{debug, warn};
use tokio::sync::Mutex;

use g3_types::metrics::MetricsName;
use g3_yaml::YamlDocPosition;

use super::{registry, ArcBackend};
use crate::config::backend::{AnyBackendConfig, BackendConfigDiffAction};

use super::dummy_close::DummyCloseBackend;
use super::stream_tcp::StreamTcpBackend;

static BACKEND_OPS_LOCK: Mutex<()> = Mutex::const_new(());

pub async fn load_all() -> anyhow::Result<()> {
    let _guard = BACKEND_OPS_LOCK.lock().await;

    let mut new_names = HashSet::<MetricsName>::new();

    let all_config = crate::config::backend::get_all();
    for config in all_config {
        let name = config.name();
        new_names.insert(name.clone());
        match registry::get_config(name) {
            Some(old) => {
                debug!("reloading backend {name}");
                reload_unlocked(old, config.as_ref().clone()).await?;
                debug!("backend {name} reload OK");
            }
            None => {
                debug!("creating backend {name}");
                spawn_new_unlocked(config.as_ref().clone()).await?;
                debug!("backend {name} create OK");
            }
        }
    }

    for name in &registry::get_names() {
        if !new_names.contains(name) {
            debug!("deleting backend {name}");
            registry::del(name);
            crate::serve::update_dependency_to_backend(name, "deleted").await;
            debug!("backend {name} deleted");
        }
    }

    Ok(())
}

pub(crate) async fn reload(
    name: &MetricsName,
    position: Option<YamlDocPosition>,
) -> anyhow::Result<()> {
    let _guard = BACKEND_OPS_LOCK.lock().await;

    let old_config = match registry::get_config(name) {
        Some(config) => config,
        None => return Err(anyhow!("no backend with name {name} found")),
    };

    let position = match position {
        Some(position) => position,
        None => match old_config.position() {
            Some(position) => position,
            None => {
                return Err(anyhow!(
                    "no config position for backend {name} found, reload is not supported"
                ));
            }
        },
    };

    let position2 = position.clone();
    let config =
        tokio::task::spawn_blocking(move || crate::config::backend::load_at_position(&position2))
            .await
            .map_err(|e| anyhow!("unable to join conf load task: {e}"))?
            .context(format!("unload to load conf at position {position}"))?;
    if name != config.name() {
        return Err(anyhow!(
            "backend at position {position} has name {}, while we expect {name}",
            config.name()
        ));
    }

    debug!("reloading backend {name} from position {position}");
    reload_unlocked(old_config, config).await?;
    debug!("backend {name} reload OK");
    Ok(())
}

pub(crate) async fn update_dependency_to_discover(discover: &MetricsName, status: &str) {
    let _guard = BACKEND_OPS_LOCK.lock().await;

    let mut backends = Vec::<ArcBackend>::new();

    registry::foreach(|_name, backend| {
        if backend.discover().eq(discover) {
            backends.push(backend.clone());
        }
    });

    if backends.is_empty() {
        return;
    }

    debug!("discover {discover} changed({status}), will reload backend(s)");
    for backend in backends {
        let name = backend.name();
        debug!("backend {name}: will update discover {discover}");
        if let Err(e) = backend.update_discover() {
            warn!("failed to update discover {discover} for backend {name}: {e:?}",);
        }
    }
}

async fn reload_unlocked(old: AnyBackendConfig, new: AnyBackendConfig) -> anyhow::Result<()> {
    let name = old.name();
    match old.diff_action(&new) {
        BackendConfigDiffAction::NoAction => {
            debug!("backend {name} reload: no action is needed");
            Ok(())
        }
        BackendConfigDiffAction::SpawnNew => {
            debug!("backend {name} reload: will create a totally new one");
            spawn_new_unlocked(new).await
        }
        BackendConfigDiffAction::Reload => {
            debug!("backend {name} reload: will reload from existed");
            reload_existed_unlocked(name, Some(new)).await
        }
        BackendConfigDiffAction::UpdateInPlace(flags) => {
            debug!("backend {name} reload: will update the existed in place");
            registry::update_config_in_place(name, flags, new)
        }
    }
}

async fn reload_existed_unlocked(
    name: &MetricsName,
    new: Option<AnyBackendConfig>,
) -> anyhow::Result<()> {
    registry::reload_existed(name, new).await?;
    crate::serve::update_dependency_to_backend(name, "reloaded").await;
    Ok(())
}

async fn spawn_new_unlocked(config: AnyBackendConfig) -> anyhow::Result<()> {
    let name = config.name().clone();
    let site = match config {
        AnyBackendConfig::DummyClose(c) => DummyCloseBackend::prepare_initial(c)?,
        AnyBackendConfig::StreamTcp(c) => StreamTcpBackend::prepare_initial(c)?,
    };
    registry::add(name.clone(), site);
    crate::serve::update_dependency_to_backend(&name, "spawned").await;
    Ok(())
}
