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
use once_cell::sync::Lazy;
use tokio::sync::Mutex;

use g3_yaml::YamlDocPosition;

use super::registry;
use crate::auth::UserGroup;
use crate::config::auth::UserGroupConfig;

static USER_GROUP_OPS_LOCK: Lazy<Mutex<()>> = Lazy::new(|| Mutex::new(()));

pub async fn load_all() -> anyhow::Result<()> {
    let _guard = USER_GROUP_OPS_LOCK.lock().await;

    let mut new_names = HashSet::<String>::new();

    let all_config = crate::config::auth::get_all();
    for config in all_config {
        let name = config.name();
        new_names.insert(name.to_string());
        match registry::get_config(name) {
            Some(old) => {
                debug!("reloading user group {name}");
                reload_old_unlocked(old, config.as_ref().clone()).await?;
                debug!("user group {name} reload OK");
            }
            None => {
                debug!("creating user group {name}");
                spawn_new_unlocked(config.as_ref().clone()).await?;
                debug!("user group {name} create OK");
            }
        }
    }

    for name in &registry::get_names() {
        if !new_names.contains(name) {
            debug!("deleting user group {name}");
            registry::del(name);
            crate::serve::update_dependency_to_user_group(name, "deleted").await;
            debug!("user group {name} deleted");
        }
    }

    Ok(())
}

pub(crate) async fn reload(name: &str, position: Option<YamlDocPosition>) -> anyhow::Result<()> {
    let _guard = USER_GROUP_OPS_LOCK.lock().await;

    let old_config = match registry::get_config(name) {
        Some(config) => config,
        None => return Err(anyhow!("no user group with name {name} found")),
    };

    let position = match position {
        Some(position) => position,
        None => match old_config.position() {
            Some(position) => position,
            None => {
                return Err(anyhow!(
                    "no config position for user group {name} found, reload is not supported"
                ));
            }
        },
    };

    let position2 = position.clone();
    let config =
        tokio::task::spawn_blocking(move || crate::config::auth::load_at_position(&position2))
            .await
            .map_err(|e| anyhow!("unable to join conf load task: {e}"))?
            .context(format!("unload to load conf at position {position}"))?;
    if name != config.name() {
        return Err(anyhow!(
            "user group at position {position} has name {}, while we expect {name}",
            config.name()
        ));
    }

    debug!("reloading user group {name} from position {position}");
    reload_old_unlocked(old_config, config).await?;
    debug!("user group {name} reload OK");
    Ok(())
}

async fn reload_old_unlocked(old: UserGroupConfig, new: UserGroupConfig) -> anyhow::Result<()> {
    let name = old.name();
    // the reload check is done inside the group code, not through config
    registry::reload_existed(name, Some(new))?;
    crate::serve::update_dependency_to_user_group(name, "reloaded").await;
    Ok(())
}

async fn spawn_new_unlocked(config: UserGroupConfig) -> anyhow::Result<()> {
    let name = config.name().to_string();
    let group = UserGroup::new_with_config(config).await?;
    registry::add(name.clone(), group);
    crate::serve::update_dependency_to_user_group(&name, "spawned").await;
    Ok(())
}
