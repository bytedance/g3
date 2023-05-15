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

use g3_types::metrics::MetricsName;
use g3_yaml::YamlDocPosition;

use super::registry;
use crate::audit::Auditor;
use crate::config::audit::AuditorConfig;

static AUDITOR_OPS_LOCK: Lazy<Mutex<()>> = Lazy::new(|| Mutex::new(()));

pub async fn load_all() -> anyhow::Result<()> {
    let _guard = AUDITOR_OPS_LOCK.lock().await;

    let mut new_names = HashSet::<MetricsName>::new();

    let all_config = crate::config::audit::get_all();
    for config in all_config {
        let name = config.name();
        new_names.insert(name.clone());
        match registry::get_config(name) {
            Some(old) => {
                debug!("reloading auditor {name}");
                reload_old_unlocked(old, config.as_ref().clone()).await?;
                debug!("auditor {name} reload OK");
            }
            None => {
                debug!("creating auditor {name}");
                spawn_new_unlocked(config.as_ref().clone()).await?;
                debug!("auditor {name} create OK");
            }
        }
    }

    for name in &registry::get_names() {
        if !new_names.contains(name) {
            debug!("deleting auditor {name}");
            registry::del(name);
            crate::serve::update_dependency_to_auditor(name, "deleted").await;
            debug!("auditor {name} deleted");
        }
    }

    Ok(())
}

pub(crate) async fn reload(
    name: &MetricsName,
    position: Option<YamlDocPosition>,
) -> anyhow::Result<()> {
    let _guard = AUDITOR_OPS_LOCK.lock().await;

    let old_config = match registry::get_config(name) {
        Some(config) => config,
        None => return Err(anyhow!("no auditor with name {name} found")),
    };

    let position = match position {
        Some(position) => position,
        None => match old_config.position() {
            Some(position) => position,
            None => {
                return Err(anyhow!(
                    "no config position for auditor {name} found, reload is not supported"
                ));
            }
        },
    };

    let position2 = position.clone();
    let config =
        tokio::task::spawn_blocking(move || crate::config::audit::load_at_position(&position2))
            .await
            .map_err(|e| anyhow!("unable to join conf load task: {e}"))?
            .context(format!("unload to load conf at position {position}"))?;
    if name != config.name() {
        return Err(anyhow!(
            "auditor at position {position} has name {}, while we expect {name}",
            config.name()
        ));
    }

    debug!("reloading auditor {name} from position {position}");
    reload_old_unlocked(old_config, config).await?;
    debug!("auditor {name} reload OK");
    Ok(())
}

async fn reload_old_unlocked(old: AuditorConfig, new: AuditorConfig) -> anyhow::Result<()> {
    let name = old.name();
    let Some(old_auditor) = registry::get(name) else {
        return Err(anyhow!("no auditor with name {name} found"));
    };
    let new_auditor = old_auditor.reload(new);
    registry::add(name.clone(), new_auditor);
    crate::serve::update_dependency_to_auditor(name, "reloaded").await;
    Ok(())
}

async fn spawn_new_unlocked(config: AuditorConfig) -> anyhow::Result<()> {
    let name = config.name().clone();
    let auditor = Auditor::new_with_config(config);
    registry::add(name.clone(), auditor);
    crate::serve::update_dependency_to_auditor(&name, "spawned").await;
    Ok(())
}
