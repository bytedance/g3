/*
 * SPDX-License-Identifier: Apache-2.0
 * Copyright 2023-2025 ByteDance and/or its affiliates.
 */

use std::sync::Mutex;

use g3_types::stats::GlobalStatsMap;

pub fn move_ht<T>(in_ht_lock: &Mutex<GlobalStatsMap<T>>, out_ht_lock: &Mutex<GlobalStatsMap<T>>) {
    let mut tmp_req_map = GlobalStatsMap::new();
    let mut in_req_map = in_ht_lock.lock().unwrap();
    for (k, v) in in_req_map.drain() {
        tmp_req_map.insert(k, v);
    }
    drop(in_req_map); // drop early

    if !tmp_req_map.is_empty() {
        let mut out_req_map = out_ht_lock.lock().unwrap();
        for (k, v) in tmp_req_map.drain() {
            out_req_map.insert(k, v);
        }
    }
}
