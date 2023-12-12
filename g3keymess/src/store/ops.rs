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

use anyhow::Context;
use tokio::sync::Mutex;

use g3_types::metrics::MetricsName;

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
            .context(format!("failed to spawn subscriber for key store {name}",))?
        {
            registry::add_subscriber(name, sender);
        }
    }

    Ok(())
}

pub async fn reload_all() -> anyhow::Result<()> {
    let _guard = KEY_STORE_OPS_LOCK.lock().await;

    let mut new_names = HashSet::<MetricsName>::new();

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
            .context(format!("failed to spawn subscriber for key store {name}",))?
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
