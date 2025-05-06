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
use crate::config::exporter::{AnyExporterConfig, ExporterConfigDiffAction};

static EXPORTER_OPS_LOCK: Mutex<()> = Mutex::const_new(());

pub async fn load_all() -> anyhow::Result<()> {
    let _guard = EXPORTER_OPS_LOCK.lock().await;

    let mut new_names = HashSet::<NodeName>::new();

    let all_config = crate::config::exporter::get_all();
    for config in all_config {
        let name = config.name();
        new_names.insert(name.clone());
        match registry::get_config(name) {
            Some(old) => {
                debug!("reloading exporter {name}");
                reload_unlocked(old, config.as_ref().clone()).await?;
                debug!("exporter {name} reload OK");
            }
            None => {
                debug!("creating exporter {name}");
                spawn_new_unlocked(config.as_ref().clone()).await?;
                debug!("exporter {name} create OK");
            }
        }
    }

    for name in &registry::get_names() {
        if !new_names.contains(name) {
            debug!("deleting exporter {name}");
            delete_existed_unlocked(name).await;
            debug!("exporter {name} deleted");
        }
    }

    Ok(())
}

pub(crate) async fn reload(
    name: &NodeName,
    position: Option<YamlDocPosition>,
) -> anyhow::Result<()> {
    let _guard = EXPORTER_OPS_LOCK.lock().await;

    let old_config = match registry::get_config(name) {
        Some(config) => config,
        None => return Err(anyhow!("no exporter with name {name} found")),
    };

    let position = match position {
        Some(position) => position,
        None => match old_config.position() {
            Some(position) => position,
            None => {
                return Err(anyhow!(
                    "no config position for exporter {name} found, reload is not supported"
                ));
            }
        },
    };

    let position2 = position.clone();
    let config =
        tokio::task::spawn_blocking(move || crate::config::exporter::load_at_position(&position2))
            .await
            .map_err(|e| anyhow!("unable to join conf load task: {e}"))?
            .context(format!("unload to load conf at position {position}"))?;
    if name != config.name() {
        return Err(anyhow!(
            "exporter at position {position} has name {}, while we expect {name}",
            config.name()
        ));
    }

    debug!("reloading exporter {name} from position {position}");
    reload_unlocked(old_config, config).await?;
    debug!("exporter {name} reload OK");
    Ok(())
}

async fn reload_unlocked(old: AnyExporterConfig, new: AnyExporterConfig) -> anyhow::Result<()> {
    let name = old.name();
    match old.diff_action(&new) {
        ExporterConfigDiffAction::NoAction => {
            debug!("exporter {name} reload: no action is needed");
            Ok(())
        }
        ExporterConfigDiffAction::SpawnNew => {
            debug!("exporter {name} reload: will create a totally new one");
            spawn_new_unlocked(new).await
        }
        ExporterConfigDiffAction::Reload => {
            debug!("exporter {name} reload: will reload from existed");
            reload_existed_unlocked(name, Some(new)).await
        }
    }
}

async fn delete_existed_unlocked(name: &NodeName) {
    const STATUS: &str = "deleted";

    registry::del(name);
    crate::collect::update_dependency_to_exporter(name, STATUS).await;
}

async fn reload_existed_unlocked(
    name: &NodeName,
    new: Option<AnyExporterConfig>,
) -> anyhow::Result<()> {
    const STATUS: &str = "reloaded";

    registry::reload_existed(name, new)?;
    crate::collect::update_dependency_to_exporter(name, STATUS).await;
    Ok(())
}

// use async fn to allow tokio schedule
async fn spawn_new_unlocked(config: AnyExporterConfig) -> anyhow::Result<()> {
    const STATUS: &str = "spawned";

    let exporter = match config {
        AnyExporterConfig::Discard(config) => {
            super::discard::DiscardExporter::prepare_initial(config)
        }
        AnyExporterConfig::Console(config) => {
            super::console::ConsoleExporter::prepare_initial(config)
        }
        AnyExporterConfig::Memory(config) => super::memory::MemoryExporter::prepare_initial(config),
        AnyExporterConfig::Graphite(config) => {
            super::graphite::GraphiteExporter::prepare_initial(config)
        }
        AnyExporterConfig::Opentsdb(config) => {
            super::opentsdb::OpentsdbExporter::prepare_initial(config)?
        }
        AnyExporterConfig::Influxdb(config) => {
            super::influxdb::InfluxdbExporter::prepare_initial(config)?
        }
    };
    let name = exporter.name().clone();
    registry::add(exporter);
    crate::collect::update_dependency_to_exporter(&name, STATUS).await;
    Ok(())
}
