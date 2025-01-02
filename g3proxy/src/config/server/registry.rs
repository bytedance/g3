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
use std::sync::{Arc, LazyLock, Mutex};

use g3_types::metrics::MetricsName;

use super::AnyServerConfig;

static INITIAL_SERVER_CONFIG_REGISTRY: LazyLock<Mutex<HashMap<MetricsName, Arc<AnyServerConfig>>>> =
    LazyLock::new(|| Mutex::new(HashMap::new()));

pub(crate) fn clear() {
    let mut ht = INITIAL_SERVER_CONFIG_REGISTRY.lock().unwrap();
    ht.clear();
}

pub(super) fn add(server: AnyServerConfig) -> Option<AnyServerConfig> {
    let name = server.name().clone();
    let server = Arc::new(server);
    let mut ht = INITIAL_SERVER_CONFIG_REGISTRY.lock().unwrap();
    ht.insert(name, server).map(|v| v.as_ref().clone())
}

pub(super) fn del(name: &MetricsName) {
    let mut ht = INITIAL_SERVER_CONFIG_REGISTRY.lock().unwrap();
    ht.remove(name);
}

pub(super) fn get(name: &MetricsName) -> Option<Arc<AnyServerConfig>> {
    let ht = INITIAL_SERVER_CONFIG_REGISTRY.lock().unwrap();
    ht.get(name).cloned()
}

pub(super) fn get_all_names() -> Vec<MetricsName> {
    let ht = INITIAL_SERVER_CONFIG_REGISTRY.lock().unwrap();
    ht.keys().cloned().collect()
}
