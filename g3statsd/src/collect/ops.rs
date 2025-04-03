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

use std::collections::HashSet;

use anyhow::{Context, anyhow};
use log::debug;
use tokio::sync::Mutex;

use g3_types::metrics::NodeName;
use g3_yaml::YamlDocPosition;

use super::{ArcCollector, registry};
use crate::config::collector::{AnyCollectorConfig, CollectorConfigDiffAction};

static COLLECTOR_OPS_LOCK: Mutex<()> = Mutex::const_new(());

pub async fn spawn_all() -> anyhow::Result<()> {
    let _guard = COLLECTOR_OPS_LOCK.lock().await;

    let mut new_names = HashSet::<NodeName>::new();

    let all_config = crate::config::collector::get_all_sorted()?;
    for config in all_config {
        let name = config.name();
        new_names.insert(name.clone());
        match registry::get_config(name) {
            Some(old) => {
                debug!("reloading collector {name}");
                reload_old_unlocked(old, config.as_ref().clone()).await?;
                debug!("collector {name} reload OK");
            }
            None => {
                debug!("creating collector {name}");
                spawn_new_unlocked(config.as_ref().clone()).await?;
                debug!("collector {name} create OK");
            }
        }
    }

    for name in &registry::get_names() {
        if !new_names.contains(name) {
            debug!("deleting collector {name}");
            delete_existed_unlocked(name).await;
            debug!("collector {name} deleted");
        }
    }

    Ok(())
}

pub async fn stop_all() {
    let _guard = COLLECTOR_OPS_LOCK.lock().await;

    registry::foreach(|_name, collect| {
        collect._abort_runtime();
    });
}

pub(crate) async fn reload(
    name: &NodeName,
    position: Option<YamlDocPosition>,
) -> anyhow::Result<()> {
    let _guard = COLLECTOR_OPS_LOCK.lock().await;

    let old_config = match registry::get_config(name) {
        Some(config) => config,
        None => return Err(anyhow!("no collector with name {name} found")),
    };

    let position = match position {
        Some(position) => position,
        None => match old_config.position() {
            Some(position) => position,
            None => {
                return Err(anyhow!(
                    "no config position for collector {name} found, reload is not supported"
                ));
            }
        },
    };

    let position2 = position.clone();
    let config =
        tokio::task::spawn_blocking(move || crate::config::collector::load_at_position(&position2))
            .await
            .map_err(|e| anyhow!("unable to join conf load task: {e}"))?
            .context(format!("unload to load conf at position {position}"))?;
    if name != config.name() {
        return Err(anyhow!(
            "collector at position {position} has name {}, while we expect {name}",
            config.name()
        ));
    }

    debug!("reloading collector {name} from position {position}");
    reload_old_unlocked(old_config, config).await?;
    debug!("collector {name} reload OK");
    Ok(())
}

async fn reload_old_unlocked(
    old: AnyCollectorConfig,
    new: AnyCollectorConfig,
) -> anyhow::Result<()> {
    let name = old.name();
    match old.diff_action(&new) {
        CollectorConfigDiffAction::NoAction => {
            debug!("collector {name} reload: no action is needed");
            Ok(())
        }
        CollectorConfigDiffAction::SpawnNew => {
            debug!("collector {name} reload: will create a totally new one");
            spawn_new_unlocked(new).await
        }
        CollectorConfigDiffAction::ReloadNoRespawn => {
            debug!("collector {name} reload: will reload config without respawn");
            registry::reload_no_respawn(name, new)?;
            update_dependency_to_collector_unlocked(name, "reloaded");
            crate::import::update_dependency_to_collector(name, "reloaded").await;
            Ok(())
        }
        CollectorConfigDiffAction::ReloadAndRespawn => {
            debug!("collector {name} reload: will respawn with old stats");
            registry::reload_and_respawn(name, new)?;
            update_dependency_to_collector_unlocked(name, "respawned");
            crate::import::update_dependency_to_collector(name, "respawned").await;
            Ok(())
        }
    }
}

fn update_dependency_to_collector_unlocked(target: &NodeName, status: &str) {
    let mut collectors = Vec::<ArcCollector>::new();

    registry::foreach(|_name, collect| {
        if collect._depend_on_collector(target) {
            collectors.push(collect.clone());
        }
    });

    if collectors.is_empty() {
        return;
    }

    debug!(
        "collector {target} changed({status}), will reload {} collector(s)",
        collectors.len()
    );
    for collector in collectors.iter() {
        debug!(
            "collector {}: will reload next collectors as it's using collector {target}",
            collector.name()
        );
        collector._update_next_collectors_in_place();
    }
}

async fn delete_existed_unlocked(name: &NodeName) {
    const STATUS: &str = "deleted";

    registry::del(name);
    update_dependency_to_collector_unlocked(name, STATUS);
    crate::import::update_dependency_to_collector(name, STATUS).await;
}

// use async fn to allow tokio schedule
async fn spawn_new_unlocked(config: AnyCollectorConfig) -> anyhow::Result<()> {
    const STATUS: &str = "spawned";

    let name = config.name().clone();
    let collector = match config {
        AnyCollectorConfig::Dummy(config) => super::dummy::DummyCollector::prepare_initial(config)?,
        AnyCollectorConfig::Internal(config) => {
            super::internal::InternalCollector::prepare_initial(config)?
        }
        AnyCollectorConfig::Regulate(config) => {
            super::regulate::RegulateCollector::prepare_initial(config)?
        }
    };
    registry::add(name.clone(), collector)?;
    update_dependency_to_collector_unlocked(&name, STATUS);
    crate::import::update_dependency_to_collector(&name, STATUS).await;
    Ok(())
}
