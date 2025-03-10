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

use super::{ArcCollect, registry};
use crate::config::collect::{AnyCollectConfig, CollectConfigDiffAction};

static COLLECT_OPS_LOCK: Mutex<()> = Mutex::const_new(());

pub async fn spawn_all() -> anyhow::Result<()> {
    let _guard = COLLECT_OPS_LOCK.lock().await;

    let mut new_names = HashSet::<NodeName>::new();

    let all_config = crate::config::collect::get_all_sorted()?;
    for config in all_config {
        let name = config.name();
        new_names.insert(name.clone());
        match registry::get_config(name) {
            Some(old) => {
                debug!("reloading collect {name}");
                reload_old_unlocked(old, config.as_ref().clone())?;
                debug!("collect {name} reload OK");
            }
            None => {
                debug!("creating collect {name}");
                spawn_new_unlocked(config.as_ref().clone())?;
                debug!("collect {name} create OK");
            }
        }
    }

    for name in &registry::get_names() {
        if !new_names.contains(name) {
            debug!("deleting collect {name}");
            delete_existed_unlocked(name);
            debug!("collect {name} deleted");
        }
    }

    Ok(())
}

pub async fn stop_all() {
    let _guard = COLLECT_OPS_LOCK.lock().await;

    registry::foreach(|_name, collect| {
        collect._abort_runtime();
    });
}

pub(crate) async fn reload(
    name: &NodeName,
    position: Option<YamlDocPosition>,
) -> anyhow::Result<()> {
    let _guard = COLLECT_OPS_LOCK.lock().await;

    let old_config = match registry::get_config(name) {
        Some(config) => config,
        None => return Err(anyhow!("no collect with name {name} found")),
    };

    let position = match position {
        Some(position) => position,
        None => match old_config.position() {
            Some(position) => position,
            None => {
                return Err(anyhow!(
                    "no config position for collect {name} found, reload is not supported"
                ));
            }
        },
    };

    let position2 = position.clone();
    let config =
        tokio::task::spawn_blocking(move || crate::config::collect::load_at_position(&position2))
            .await
            .map_err(|e| anyhow!("unable to join conf load task: {e}"))?
            .context(format!("unload to load conf at position {position}"))?;
    if name != config.name() {
        return Err(anyhow!(
            "collect at position {position} has name {}, while we expect {name}",
            config.name()
        ));
    }

    debug!("reloading collect {name} from position {position}");
    reload_old_unlocked(old_config, config)?;
    debug!("collect {name} reload OK");
    Ok(())
}

fn reload_old_unlocked(old: AnyCollectConfig, new: AnyCollectConfig) -> anyhow::Result<()> {
    let name = old.name();
    match old.diff_action(&new) {
        CollectConfigDiffAction::NoAction => {
            debug!("collect {name} reload: no action is needed");
            Ok(())
        }
        CollectConfigDiffAction::SpawnNew => {
            debug!("collect {name} reload: will create a totally new one");
            spawn_new_unlocked(new)
        }
        CollectConfigDiffAction::ReloadOnlyConfig => {
            debug!("collect {name} reload: will only reload config");
            registry::reload_only_config(name, new)?;
            update_dependency_to_collector_unlocked(name, "reloaded");
            Ok(())
        }
        CollectConfigDiffAction::ReloadAndRespawn => {
            debug!("collect {name} reload: will respawn with old stats");
            registry::reload_and_respawn(name, new)?;
            update_dependency_to_collector_unlocked(name, "reloaded");
            Ok(())
        }
    }
}

fn update_dependency_to_collector_unlocked(target: &NodeName, status: &str) {
    let mut collectors = Vec::<ArcCollect>::new();

    registry::foreach(|_name, collect| {
        if collect._depend_on_collector(target) {
            collectors.push(collect.clone());
        }
    });

    if collectors.is_empty() {
        return;
    }

    debug!(
        "collect {target} changed({status}), will reload {} collect(s)",
        collectors.len()
    );
    for collect in collectors.iter() {
        debug!(
            "collect {}: will reload next collectors as it's using collect {target}",
            collect.name()
        );
        collect._update_next_collectors_in_place();
    }
}

fn delete_existed_unlocked(name: &NodeName) {
    registry::del(name);
    update_dependency_to_collector_unlocked(name, "deleted");
}

// use async fn to allow tokio schedule
fn spawn_new_unlocked(config: AnyCollectConfig) -> anyhow::Result<()> {
    let name = config.name().clone();
    let input = match config {
        AnyCollectConfig::Dummy(config) => super::dummy::DummyCollect::prepare_initial(config)?,
        AnyCollectConfig::Internal(config) => todo!(),
    };
    registry::add(name.clone(), input)?;
    update_dependency_to_collector_unlocked(&name, "spawned");
    Ok(())
}
