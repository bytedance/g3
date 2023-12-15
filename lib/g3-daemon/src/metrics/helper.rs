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

use std::hash::Hash;
use std::sync::Mutex;

use ahash::AHashMap;

pub fn move_ht<K, V>(in_ht_lock: &Mutex<AHashMap<K, V>>, out_ht_lock: &Mutex<AHashMap<K, V>>)
where
    K: Hash + Eq,
{
    let mut tmp_req_map = AHashMap::new();
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
