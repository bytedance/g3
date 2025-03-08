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

use std::collections::HashMap;
use std::sync::{Arc, Mutex};

use foldhash::fast::FixedState;

use g3_types::metrics::NodeName;

use super::AnyInputConfig;

static INITIAL_INPUT_CONFIG_REGISTRY: Mutex<HashMap<NodeName, Arc<AnyInputConfig>, FixedState>> =
    Mutex::new(HashMap::with_hasher(FixedState::with_seed(0)));

pub(crate) fn clear() {
    let mut ht = INITIAL_INPUT_CONFIG_REGISTRY.lock().unwrap();
    ht.clear();
}

pub(super) fn add(input: AnyInputConfig) -> Option<AnyInputConfig> {
    let name = input.name().clone();
    let input = Arc::new(input);
    let mut ht = INITIAL_INPUT_CONFIG_REGISTRY.lock().unwrap();
    ht.insert(name, input).map(|v| v.as_ref().clone())
}

pub(super) fn del(name: &NodeName) {
    let mut ht = INITIAL_INPUT_CONFIG_REGISTRY.lock().unwrap();
    ht.remove(name);
}

pub(super) fn get(name: &NodeName) -> Option<Arc<AnyInputConfig>> {
    let ht = INITIAL_INPUT_CONFIG_REGISTRY.lock().unwrap();
    ht.get(name).cloned()
}

pub(crate) fn get_all() -> Vec<Arc<AnyInputConfig>> {
    let ht = INITIAL_INPUT_CONFIG_REGISTRY.lock().unwrap();
    ht.values().cloned().collect()
}
