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

use std::collections::HashMap;
use std::sync::{Arc, Mutex};

use anyhow::anyhow;
use once_cell::sync::Lazy;

use g3_types::metrics::MetricsName;

use super::AnyServerConfig;

static INITIAL_SERVER_CONFIG_REGISTRY: Lazy<Mutex<HashMap<MetricsName, Arc<AnyServerConfig>>>> =
    Lazy::new(|| Mutex::new(HashMap::new()));

pub(crate) fn clear() {
    let mut ht = INITIAL_SERVER_CONFIG_REGISTRY.lock().unwrap();
    ht.clear();
}

pub(super) fn add(server: AnyServerConfig, replace: bool) -> anyhow::Result<()> {
    let name = server.name().clone();
    let server = Arc::new(server);
    let mut ht = INITIAL_SERVER_CONFIG_REGISTRY.lock().unwrap();
    if let Some(old) = ht.insert(name, server) {
        if replace {
            Ok(())
        } else {
            Err(anyhow!(
                "server with the same name {} is already existed",
                old.name()
            ))
        }
    } else {
        Ok(())
    }
}

pub(super) fn get_all() -> Vec<Arc<AnyServerConfig>> {
    let mut vec = Vec::new();
    let ht = INITIAL_SERVER_CONFIG_REGISTRY.lock().unwrap();
    for v in ht.values() {
        vec.push(Arc::clone(v));
    }
    vec
}
