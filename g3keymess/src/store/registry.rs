/*
 * SPDX-License-Identifier: Apache-2.0
 * Copyright 2023-2025 ByteDance and/or its affiliates.
 */

use std::collections::{HashMap, HashSet};
use std::sync::Mutex;

use foldhash::fast::FixedState;
use tokio::sync::oneshot;

use g3_types::metrics::NodeName;

static KEY_STORE_SUBSCRIBER_REGISTRY: Mutex<HashMap<NodeName, oneshot::Sender<()>, FixedState>> =
    Mutex::new(HashMap::with_hasher(FixedState::with_seed(0)));

pub(super) fn add_subscriber(store: NodeName, sender: oneshot::Sender<()>) {
    let mut map = KEY_STORE_SUBSCRIBER_REGISTRY.lock().unwrap();
    map.insert(store, sender);
}

pub(super) fn del_subscriber(store: &NodeName) {
    let mut map = KEY_STORE_SUBSCRIBER_REGISTRY.lock().unwrap();
    map.remove(store);
}

pub(super) fn all_subscribers() -> HashSet<NodeName> {
    let map = KEY_STORE_SUBSCRIBER_REGISTRY.lock().unwrap();
    map.keys().cloned().collect()
}
