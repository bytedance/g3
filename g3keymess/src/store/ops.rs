/*
 * SPDX-License-Identifier: Apache-2.0
 * Copyright 2023-2025 ByteDance and/or its affiliates.
 */

use std::collections::HashSet;

use anyhow::Context;
use tokio::sync::Mutex;

use g3_types::metrics::NodeName;

use super::registry;

static KEY_STORE_OPS_LOCK: Mutex<()> = Mutex::const_new(());

pub async fn load_all() -> anyhow::Result<()> {
    let _guard = KEY_STORE_OPS_LOCK.lock().await;

    let all_config = crate::config::store::get_all();
    for config in all_config {
        let name = config.name().clone();
        config
            .load_keys()
            .await
            .context(format!("failed to load keys for key store {name}"))?;

        if let Some(sender) = config
            .spawn_subscriber()
            .context(format!("failed to spawn subscriber for key store {name}"))?
        {
            registry::add_subscriber(name, sender);
        }
    }

    Ok(())
}

pub async fn reload_all() -> anyhow::Result<()> {
    let _guard = KEY_STORE_OPS_LOCK.lock().await;

    let mut new_names = HashSet::<NodeName>::new();

    let all_config = crate::config::store::get_all();
    for config in all_config {
        let name = config.name().clone();
        new_names.insert(name.clone());
        config
            .load_keys()
            .await
            .context(format!("failed to load keys for key store {name}"))?;

        if let Some(sender) = config
            .spawn_subscriber()
            .context(format!("failed to spawn subscriber for key store {name}"))?
        {
            registry::add_subscriber(name, sender);
        }
    }

    for name in &registry::all_subscribers() {
        if !new_names.contains(name) {
            registry::del_subscriber(name);
        }
    }

    Ok(())
}
