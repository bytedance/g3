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
use async_recursion::async_recursion;
use log::{debug, warn};
use once_cell::sync::Lazy;
use tokio::sync::Mutex;

use g3_yaml::YamlDocPosition;

use crate::config::resolver::{AnyResolverConfig, ResolverConfigDiffAction};

#[cfg(feature = "c-ares")]
use super::c_ares::CAresResolver;
use super::trust_dns::TrustDnsResolver;

use super::deny_all::DenyAllResolver;
use super::fail_over::FailOverResolver;

use super::registry;

static RESOLVER_OPS_LOCK: Lazy<Mutex<()>> = Lazy::new(|| Mutex::new(()));

pub async fn spawn_all() -> anyhow::Result<()> {
    let _guard = RESOLVER_OPS_LOCK.lock().await;

    let mut new_names = HashSet::<String>::new();

    let all_config = crate::config::resolver::get_all_sorted()?;
    for config in all_config {
        let name = config.name();
        new_names.insert(name.to_string());
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
            let old_resolver = registry::del(name);
            crate::escape::update_dependency_to_resolver(name, "deleted").await;
            if let Some(mut resolver) = old_resolver {
                resolver._shutdown().await;
            }
            debug!("resolver {name} deleted");
        }
    }

    Ok(())
}

pub(crate) async fn reload(name: &str, position: Option<YamlDocPosition>) -> anyhow::Result<()> {
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
    update_dependency_to_resolver_unlocked(name).await;
    Ok(())
}

#[async_recursion]
async fn update_dependency_to_resolver_unlocked(target: &str) {
    let mut names = Vec::<String>::new();

    registry::foreach(|name, resolver| {
        if let Some(set) = resolver._dependent_resolver() {
            if set.contains(target) {
                names.push(name.to_string())
            }
        }
    });

    debug!("resolver {target} changed, will reload resolvers(s) {names:?} which depend on it");
    for name in names.iter() {
        debug!("resolver {name}: will reload as it depends on resolver {target}");
        if let Err(e) = registry::update_dependency(name, target) {
            warn!("failed to update dependency {target} for resolver {name}: {e:?}");
        }
    }

    // finish those in the same level first, then go in depth
    for name in names.iter() {
        update_dependency_to_resolver_unlocked(name).await;
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

async fn spawn_new_unlocked(config: AnyResolverConfig) -> anyhow::Result<()> {
    let name = config.name().to_string();
    let resolver = match config {
        #[cfg(feature = "c-ares")]
        AnyResolverConfig::CAres(_) => CAresResolver::new_obj(config)?,
        AnyResolverConfig::TrustDns(_) => TrustDnsResolver::new_obj(config)?,
        AnyResolverConfig::DenyAll(_) => DenyAllResolver::new_obj(config)?,
        AnyResolverConfig::FailOver(_) => FailOverResolver::new_obj(config)?,
    };
    let old_resolver = registry::add(name.clone(), resolver);
    crate::escape::update_dependency_to_resolver(&name, "spawned").await;
    if let Some(mut resolver) = old_resolver {
        resolver._shutdown().await;
    }
    Ok(())
}
