/*
 * SPDX-License-Identifier: Apache-2.0
 * Copyright 2025 ByteDance and/or its affiliates.
 */

use std::collections::HashSet;

use anyhow::{Context, anyhow};
use log::{debug, warn};
use tokio::sync::Mutex;

use g3_types::metrics::NodeName;
use g3_yaml::YamlDocPosition;

use super::registry;
use crate::config::importer::{AnyImporterConfig, ImporterConfigDiffAction};

static IMPORTER_OPS_LOCK: Mutex<()> = Mutex::const_new(());

pub async fn spawn_all() -> anyhow::Result<()> {
    let _guard = IMPORTER_OPS_LOCK.lock().await;

    let mut new_names = HashSet::<NodeName>::new();

    let all_config = crate::config::importer::get_all();
    for config in all_config {
        let name = config.name();
        new_names.insert(name.clone());
        match registry::get_config(name) {
            Some(old) => {
                debug!("reloading importer {name}");
                reload_old_unlocked(old, config.as_ref().clone())?;
                debug!("importer {name} reload OK");
            }
            None => {
                debug!("creating importer {name}");
                spawn_new_unlocked(config.as_ref().clone())?;
                debug!("importer {name} create OK");
            }
        }
    }

    for name in &registry::get_names() {
        if !new_names.contains(name) {
            debug!("deleting importer {name}");
            delete_existed_unlocked(name);
            debug!("importer {name} deleted");
        }
    }

    Ok(())
}

pub async fn stop_all() {
    let _guard = IMPORTER_OPS_LOCK.lock().await;

    registry::foreach(|_name, importer| {
        importer._abort_runtime();
    });
}

pub(crate) async fn reload(
    name: &NodeName,
    position: Option<YamlDocPosition>,
) -> anyhow::Result<()> {
    let _guard = IMPORTER_OPS_LOCK.lock().await;

    let old_config = match registry::get_config(name) {
        Some(config) => config,
        None => return Err(anyhow!("no importer with name {name} found")),
    };

    let position = match position {
        Some(position) => position,
        None => match old_config.position() {
            Some(position) => position,
            None => {
                return Err(anyhow!(
                    "no config position for importer {name} found, reload is not supported"
                ));
            }
        },
    };

    let position2 = position.clone();
    let config =
        tokio::task::spawn_blocking(move || crate::config::importer::load_at_position(&position2))
            .await
            .map_err(|e| anyhow!("unable to join conf load task: {e}"))?
            .context(format!("unload to load conf at position {position}"))?;
    if name != config.name() {
        return Err(anyhow!(
            "importer at position {position} has name {}, while we expect {name}",
            config.name()
        ));
    }

    debug!("reloading importer {name} from position {position}");
    reload_old_unlocked(old_config, config)?;
    debug!("importer {name} reload OK");
    Ok(())
}

pub(crate) async fn update_dependency_to_collector(collector: &NodeName, status: &str) {
    let _guard = IMPORTER_OPS_LOCK.lock().await;

    let mut names = Vec::<NodeName>::new();

    registry::foreach(|name, importer| {
        if importer.collector().eq(collector) {
            names.push(name.clone());
        }
    });

    if names.is_empty() {
        return;
    }

    debug!("collector {collector} changed({status}), will reload importer(s) {names:?}");
    for name in names.iter() {
        debug!("importer {name}: will reload as it's using collector {collector}");
        if let Err(e) = registry::reload_only_collector(name) {
            warn!("failed to reload importer {name}: {e:?}");
        }
    }
}

fn reload_old_unlocked(old: AnyImporterConfig, new: AnyImporterConfig) -> anyhow::Result<()> {
    let name = old.name();
    match old.diff_action(&new) {
        ImporterConfigDiffAction::NoAction => {
            debug!("importer {name} reload: no action is needed");
            Ok(())
        }
        ImporterConfigDiffAction::SpawnNew => {
            debug!("importer {name} reload: will create a totally new one");
            spawn_new_unlocked(new)
        }
        ImporterConfigDiffAction::ReloadNoRespawn => {
            debug!("importer {name} reload: will reload config without respawn");
            registry::reload_no_respawn(name, new)?;
            Ok(())
        }
        ImporterConfigDiffAction::ReloadAndRespawn => {
            debug!("importer {name} reload: will respawn with old stats");
            registry::reload_and_respawn(name, new)?;
            Ok(())
        }
    }
}

fn delete_existed_unlocked(name: &NodeName) {
    registry::del(name);
}

// use async fn to allow tokio schedule
fn spawn_new_unlocked(config: AnyImporterConfig) -> anyhow::Result<()> {
    let importer = match config {
        AnyImporterConfig::Dummy(config) => super::dummy::DummyImporter::prepare_initial(config)?,
        AnyImporterConfig::StatsDUdp(config) => {
            super::statsd::StatsdUdpImporter::prepare_initial(config)?
        }
        #[cfg(unix)]
        AnyImporterConfig::StatsDUnix(config) => {
            super::statsd::StatsdUnixImporter::prepare_initial(config)?
        }
    };
    registry::add(importer)
}
