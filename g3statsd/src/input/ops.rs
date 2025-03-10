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

use super::registry;
use crate::config::input::{AnyInputConfig, InputConfigDiffAction};

static INPUT_OPS_LOCK: Mutex<()> = Mutex::const_new(());

pub async fn spawn_all() -> anyhow::Result<()> {
    let _guard = INPUT_OPS_LOCK.lock().await;

    let mut new_names = HashSet::<NodeName>::new();

    let all_config = crate::config::input::get_all();
    for config in all_config {
        let name = config.name();
        new_names.insert(name.clone());
        match registry::get_config(name) {
            Some(old) => {
                debug!("reloading input {name}");
                reload_old_unlocked(old, config.as_ref().clone())?;
                debug!("input {name} reload OK");
            }
            None => {
                debug!("creating input {name}");
                spawn_new_unlocked(config.as_ref().clone())?;
                debug!("input {name} create OK");
            }
        }
    }

    for name in &registry::get_names() {
        if !new_names.contains(name) {
            debug!("deleting input {name}");
            delete_existed_unlocked(name);
            debug!("input {name} deleted");
        }
    }

    Ok(())
}

pub async fn stop_all() {
    let _guard = INPUT_OPS_LOCK.lock().await;

    registry::foreach(|_name, input| {
        input._abort_runtime();
    });
}

pub(crate) async fn reload(
    name: &NodeName,
    position: Option<YamlDocPosition>,
) -> anyhow::Result<()> {
    let _guard = INPUT_OPS_LOCK.lock().await;

    let old_config = match registry::get_config(name) {
        Some(config) => config,
        None => return Err(anyhow!("no input with name {name} found")),
    };

    let position = match position {
        Some(position) => position,
        None => match old_config.position() {
            Some(position) => position,
            None => {
                return Err(anyhow!(
                    "no config position for input {name} found, reload is not supported"
                ));
            }
        },
    };

    let position2 = position.clone();
    let config =
        tokio::task::spawn_blocking(move || crate::config::input::load_at_position(&position2))
            .await
            .map_err(|e| anyhow!("unable to join conf load task: {e}"))?
            .context(format!("unload to load conf at position {position}"))?;
    if name != config.name() {
        return Err(anyhow!(
            "input at position {position} has name {}, while we expect {name}",
            config.name()
        ));
    }

    debug!("reloading input {name} from position {position}");
    reload_old_unlocked(old_config, config)?;
    debug!("input {name} reload OK");
    Ok(())
}

fn reload_old_unlocked(old: AnyInputConfig, new: AnyInputConfig) -> anyhow::Result<()> {
    let name = old.name();
    match old.diff_action(&new) {
        InputConfigDiffAction::NoAction => {
            debug!("input {name} reload: no action is needed");
            Ok(())
        }
        InputConfigDiffAction::SpawnNew => {
            debug!("input {name} reload: will create a totally new one");
            spawn_new_unlocked(new)
        }
        InputConfigDiffAction::ReloadOnlyConfig => {
            debug!("input {name} reload: will only reload config");
            registry::reload_only_config(name, new)?;
            Ok(())
        }
        InputConfigDiffAction::ReloadAndRespawn => {
            debug!("input {name} reload: will respawn with old stats");
            registry::reload_and_respawn(name, new)?;
            Ok(())
        }
    }
}

fn delete_existed_unlocked(name: &NodeName) {
    registry::del(name);
}

// use async fn to allow tokio schedule
fn spawn_new_unlocked(config: AnyInputConfig) -> anyhow::Result<()> {
    let name = config.name().clone();
    let input = match config {
        AnyInputConfig::Dummy(config) => super::dummy::DummyInput::prepare_initial(config)?,
        AnyInputConfig::StatsD(config) => super::statsd::StatsdInput::prepare_initial(config)?,
    };
    registry::add(name.clone(), input)?;
    Ok(())
}
