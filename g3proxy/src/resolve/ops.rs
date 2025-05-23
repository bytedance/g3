/*
 * SPDX-License-Identifier: Apache-2.0
 * Copyright 2023-2025 ByteDance and/or its affiliates.
 */

use std::collections::HashSet;

use anyhow::{Context, anyhow};
use async_recursion::async_recursion;
use log::{debug, warn};
use tokio::sync::Mutex;

use g3_types::metrics::NodeName;
use g3_yaml::YamlDocPosition;

use crate::config::resolver::{AnyResolverConfig, ResolverConfigDiffAction};

#[cfg(feature = "c-ares")]
use super::c_ares::CAresResolver;
use super::hickory::HickoryResolver;

use super::deny_all::DenyAllResolver;
use super::fail_over::FailOverResolver;

use super::{Resolver, registry};

static RESOLVER_OPS_LOCK: Mutex<()> = Mutex::const_new(());

pub async fn spawn_all() -> anyhow::Result<()> {
    let _guard = RESOLVER_OPS_LOCK.lock().await;

    let mut new_names = HashSet::<NodeName>::new();

    let all_config = crate::config::resolver::get_all_sorted()?;
    for config in all_config {
        let name = config.name();
        new_names.insert(name.clone());
        match registry::get_config(name) {
            Some(old) => {
                debug!("reloading resolver {name}");
                reload_old_unlocked(old, config.as_ref().clone()).await?;
                debug!("resolver {name} reload OK");
            }
            None => {
                debug!("creating resolver {name}");
                spawn_new_unlocked(config.as_ref().clone()).await?;
                debug!("resolver {name} create OK");
            }
        }
    }

    for name in &registry::get_names() {
        if !new_names.contains(name) {
            debug!("deleting resolver {name}");
            delete_existed_unlocked(name).await;
            debug!("resolver {name} deleted");
        }
    }

    Ok(())
}

pub(crate) async fn reload(
    name: &NodeName,
    position: Option<YamlDocPosition>,
) -> anyhow::Result<()> {
    let _guard = RESOLVER_OPS_LOCK.lock().await;

    let old_config = match registry::get_config(name) {
        Some(config) => config,
        None => return Err(anyhow!("no resolver with name {name} found")),
    };

    let position = match position {
        Some(position) => position,
        None => match old_config.position() {
            Some(position) => position,
            None => {
                return Err(anyhow!(
                    "no config position for resolver {name} found, reload is not supported"
                ));
            }
        },
    };

    let position2 = position.clone();
    let config =
        tokio::task::spawn_blocking(move || crate::config::resolver::load_at_position(&position2))
            .await
            .map_err(|e| anyhow!("unable to join conf load task: {e}"))?
            .context(format!("unload to load conf at position {position}"))?;
    if name != config.name() {
        return Err(anyhow!(
            "resolver at position {position} has name {}, while we expect {name}",
            config.name()
        ));
    }

    debug!("reloading resolver {name} from position {position}");
    reload_old_unlocked(old_config, config).await?;
    debug!("resolver {name} reload OK");
    Ok(())
}

pub(crate) fn foreach_resolver<F>(mut f: F)
where
    F: FnMut(&NodeName, &dyn Resolver),
{
    registry::foreach(|name, resolver| f(name, resolver.as_ref()));
}

#[async_recursion]
async fn update_dependency_to_resolver_unlocked(target: &NodeName, status: &str) {
    let mut names = Vec::<NodeName>::new();

    registry::foreach(|name, resolver| {
        if let Some(set) = resolver._dependent_resolver() {
            if set.contains(target) {
                names.push(name.clone())
            }
        }
    });

    debug!(
        "resolver {target} changed({status}), will reload resolvers(s) {names:?} which depend on it"
    );
    for name in names.iter() {
        debug!("resolver {name}: will reload as it depends on resolver {target}");
        if let Err(e) = registry::update_dependency(name, target) {
            warn!("failed to update dependency {target} for resolver {name}: {e:?}");
        }
    }

    // finish those in the same level first, then go in depth
    for name in names.iter() {
        update_dependency_to_resolver_unlocked(name, "reloaded").await;
    }
}

async fn reload_old_unlocked(old: AnyResolverConfig, new: AnyResolverConfig) -> anyhow::Result<()> {
    let name = old.name();
    match old.diff_action(&new) {
        ResolverConfigDiffAction::NoAction => {
            debug!("resolver {name} reload: no action is needed");
            Ok(())
        }
        ResolverConfigDiffAction::SpawnNew => {
            debug!("resolver {name} reload: will create a totally new one");
            spawn_new_unlocked(new).await
        }
        ResolverConfigDiffAction::Update => {
            debug!("resolver {name} reload: will update the existed in place");
            registry::update_config(name, new)
        }
    }
}

async fn delete_existed_unlocked(name: &NodeName) {
    const STATUS: &str = "deleted";

    let old_resolver = registry::del(name);
    update_dependency_to_resolver_unlocked(name, STATUS).await;
    crate::escape::update_dependency_to_resolver(name, STATUS).await;
    if let Some(mut resolver) = old_resolver {
        tokio::spawn(async move {
            resolver._shutdown().await;
        });
    }
}

async fn spawn_new_unlocked(config: AnyResolverConfig) -> anyhow::Result<()> {
    const STATUS: &str = "spawned";

    let name = config.name().clone();
    let resolver = match config {
        #[cfg(feature = "c-ares")]
        AnyResolverConfig::CAres(c) => CAresResolver::new_obj(c)?,
        AnyResolverConfig::Hickory(c) => HickoryResolver::new_obj(*c)?,
        AnyResolverConfig::DenyAll(c) => DenyAllResolver::new_obj(c)?,
        AnyResolverConfig::FailOver(c) => FailOverResolver::new_obj(c)?,
    };
    let old_resolver = registry::add(name.clone(), resolver);
    update_dependency_to_resolver_unlocked(&name, STATUS).await;
    crate::escape::update_dependency_to_resolver(&name, STATUS).await;
    if let Some(mut resolver) = old_resolver {
        tokio::spawn(async move {
            resolver._shutdown().await;
        });
    }
    Ok(())
}
