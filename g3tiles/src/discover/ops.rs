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

use anyhow::{anyhow, Context};
use log::debug;
use tokio::sync::Mutex;

use g3_types::metrics::MetricsName;
use g3_yaml::YamlDocPosition;

use super::{registry, ArcDiscover};
use crate::config::discover::{AnyDiscoverConfig, DiscoverConfigDiffAction};

use super::host_resolver::HostResolverDiscover;
use super::static_addr::StaticAddrDiscover;

static DISCOVER_OPS_LOCK: Mutex<()> = Mutex::const_new(());

pub async fn load_all() -> anyhow::Result<()> {
    let _guard = DISCOVER_OPS_LOCK.lock().await;

    let mut new_names = HashSet::<MetricsName>::new();

    let all_config = crate::config::discover::get_all();
    for config in all_config {
        let name = config.name();
        new_names.insert(name.clone());
        match registry::get_config(name) {
            Some(old) => {
                debug!("reloading discover {name}");
                reload_unlocked(old, config.as_ref().clone()).await?;
                debug!("discover {name} reload OK");
            }
            None => {
                debug!("creating discover {name}");
                spawn_new_unlocked(config.as_ref().clone()).await?;
                debug!("discover {name} create OK");
            }
        }
    }

    for name in &registry::get_names() {
        if !new_names.contains(name) {
            debug!("deleting discover {name}");
            registry::del(name);
            crate::backend::update_dependency_to_discover(name, "deleted").await;
            debug!("discover {name} deleted");
        }
    }

    Ok(())
}

pub(crate) fn get_discover(name: &MetricsName) -> anyhow::Result<ArcDiscover> {
    match registry::get(name) {
        Some(site) => Ok(site),
        None => Err(anyhow!("no discover named {name} found")),
    }
}

pub(crate) async fn reload(
    name: &MetricsName,
    position: Option<YamlDocPosition>,
) -> anyhow::Result<()> {
    let _guard = DISCOVER_OPS_LOCK.lock().await;

    let old_config = match registry::get_config(name) {
        Some(config) => config,
        None => return Err(anyhow!("no discover with name {name} found")),
    };

    let position = match position {
        Some(position) => position,
        None => match old_config.position() {
            Some(position) => position,
            None => {
                return Err(anyhow!(
                    "no config position for discover {name} found, reload is not supported"
                ));
            }
        },
    };

    let position2 = position.clone();
    let config =
        tokio::task::spawn_blocking(move || crate::config::discover::load_at_position(&position2))
            .await
            .map_err(|e| anyhow!("unable to join conf load task: {e}"))?
            .context(format!("unload to load conf at position {position}"))?;
    if name != config.name() {
        return Err(anyhow!(
            "discover at position {position} has name {}, while we expect {name}",
            config.name()
        ));
    }

    debug!("reloading discover {name} from position {position}");
    reload_unlocked(old_config, config).await?;
    debug!("discover {name} reload OK");
    Ok(())
}

async fn reload_unlocked(old: AnyDiscoverConfig, new: AnyDiscoverConfig) -> anyhow::Result<()> {
    let name = old.name();
    match old.diff_action(&new) {
        DiscoverConfigDiffAction::NoAction => {
            debug!("discover {name} reload: no action is needed");
            Ok(())
        }
        DiscoverConfigDiffAction::SpawnNew => {
            debug!("discover {name} reload: will create a totally new one");
            spawn_new_unlocked(new).await
        }
        DiscoverConfigDiffAction::UpdateInPlace => {
            debug!("discover {name} reload: will update the existed in place");
            registry::update_config_in_place(name, new)
        }
    }
}

async fn spawn_new_unlocked(config: AnyDiscoverConfig) -> anyhow::Result<()> {
    let name = config.name().clone();
    let discover = match config {
        AnyDiscoverConfig::StaticAddr(c) => StaticAddrDiscover::new_obj(c),
        AnyDiscoverConfig::HostResolver(c) => HostResolverDiscover::new_obj(c),
    };
    registry::add(name.clone(), discover);
    crate::backend::update_dependency_to_discover(&name, "spawned").await;
    Ok(())
}
