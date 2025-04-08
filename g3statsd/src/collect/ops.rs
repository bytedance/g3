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
use async_recursion::async_recursion;
use log::{debug, warn};
use tokio::sync::Mutex;

use g3_types::metrics::NodeName;
use g3_yaml::YamlDocPosition;

use super::registry;
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
                reload_unlocked(old, config.as_ref().clone()).await?;
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
    reload_unlocked(old_config, config).await?;
    debug!("collector {name} reload OK");
    Ok(())
}

#[async_recursion]
async fn update_dependency_to_collector_unlocked(target: &NodeName, status: &str) {
    let mut names = Vec::<NodeName>::new();

    registry::foreach(|name, collector| {
        if collector._depend_on_collector(target) {
            names.push(name.clone())
        }
    });

    debug!(
        "collector {target} changed({status}), will reload collector(s) {names:?} which depend on it"
    );
    for name in names.iter() {
        debug!("collector {name}: will reload as it depends on collector {target}");
        if let Err(e) = reload_existed_unlocked(name, None).await {
            warn!("failed to reload collector {name}: {e:?}");
        }
    }

    // finish those in the same level first, then go in depth
    for name in names.iter() {
        update_dependency_to_collector_unlocked(name, "reloaded").await;
    }
}

async fn reload_unlocked(old: AnyCollectorConfig, new: AnyCollectorConfig) -> anyhow::Result<()> {
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
        CollectorConfigDiffAction::Reload => {
            debug!("collector {name} reload: will reload from existed");
            reload_existed_unlocked(name, Some(new)).await
        }
    }
}

async fn delete_existed_unlocked(name: &NodeName) {
    const STATUS: &str = "deleted";

    registry::del(name);
    update_dependency_to_collector_unlocked(name, STATUS).await;
    crate::import::update_dependency_to_collector(name, STATUS).await;
}

async fn reload_existed_unlocked(
    name: &NodeName,
    new: Option<AnyCollectorConfig>,
) -> anyhow::Result<()> {
    const STATUS: &str = "reloaded";

    registry::reload_existed(name, new).await?;
    update_dependency_to_collector_unlocked(name, STATUS).await;
    crate::import::update_dependency_to_collector(name, STATUS).await;
    Ok(())
}

// use async fn to allow tokio schedule
async fn spawn_new_unlocked(config: AnyCollectorConfig) -> anyhow::Result<()> {
    const STATUS: &str = "spawned";

    let name = config.name().clone();
    let collector = match config {
        AnyCollectorConfig::Aggregate(config) => {
            super::aggregate::AggregateCollector::prepare_initial(config)?
        }
        AnyCollectorConfig::Discard(config) => {
            super::discard::DiscardCollector::prepare_initial(config)?
        }
        AnyCollectorConfig::Internal(config) => {
            super::internal::InternalCollector::prepare_initial(config)?
        }
        AnyCollectorConfig::Regulate(config) => {
            super::regulate::RegulateCollector::prepare_initial(config)?
        }
    };
    registry::add(name.clone(), collector);
    update_dependency_to_collector_unlocked(&name, STATUS).await;
    crate::import::update_dependency_to_collector(&name, STATUS).await;
    Ok(())
}
