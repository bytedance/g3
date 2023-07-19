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

use std::collections::{HashMap, HashSet};
use std::sync::Mutex;

use once_cell::sync::Lazy;
use tokio::sync::oneshot;

use g3_types::metrics::MetricsName;

static KEY_STORE_SUBSCRIBER_REGISTRY: Lazy<Mutex<HashMap<MetricsName, oneshot::Sender<()>>>> =
    Lazy::new(|| Mutex::new(HashMap::new()));

pub(super) fn add_subscriber(store: MetricsName, sender: oneshot::Sender<()>) {
    let mut map = KEY_STORE_SUBSCRIBER_REGISTRY.lock().unwrap();
    map.insert(store, sender);
}

pub(super) fn del_subscriber(store: &MetricsName) {
    let mut map = KEY_STORE_SUBSCRIBER_REGISTRY.lock().unwrap();
    map.remove(store);
}

pub(super) fn all_subscribers() -> HashSet<MetricsName> {
    let map = KEY_STORE_SUBSCRIBER_REGISTRY.lock().unwrap();
    map.keys().cloned().collect()
}
