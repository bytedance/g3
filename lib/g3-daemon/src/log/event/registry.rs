/*
 * SPDX-License-Identifier: Apache-2.0
 * Copyright 2023-2025 ByteDance and/or its affiliates.
 */

use std::collections::HashMap;
use std::sync::{Arc, Mutex};

use foldhash::fast::FixedState;

use super::LoggerStats;

static RUNTIME_LOGGER_REGISTRY: Mutex<HashMap<String, Arc<LoggerStats>, FixedState>> =
    Mutex::new(HashMap::with_hasher(FixedState::with_seed(0)));

pub(super) fn add(name: String, stats: Arc<LoggerStats>) {
    let mut ht = RUNTIME_LOGGER_REGISTRY.lock().unwrap();
    let _ = ht.insert(name, stats);
}

pub(super) fn foreach_stats<F>(mut f: F)
where
    F: FnMut(&str, &Arc<LoggerStats>),
{
    let ht = RUNTIME_LOGGER_REGISTRY.lock().unwrap();
    for (name, server) in ht.iter() {
        f(name, server)
    }
}
