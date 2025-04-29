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
use std::collections::hash_map::Drain;

use foldhash::fast::FixedState;

use super::StatId;

pub struct GlobalStatsMap<T> {
    inner: HashMap<StatId, T, FixedState>,
}

impl<T> Default for GlobalStatsMap<T> {
    fn default() -> Self {
        Self::new()
    }
}

impl<T> GlobalStatsMap<T> {
    pub const fn new() -> Self {
        GlobalStatsMap {
            inner: HashMap::with_hasher(FixedState::with_seed(0)),
        }
    }

    #[inline]
    pub fn is_empty(&self) -> bool {
        self.inner.is_empty()
    }

    #[inline]
    pub fn insert(&mut self, stat_id: StatId, v: T) -> Option<T> {
        self.inner.insert(stat_id, v)
    }

    #[inline]
    pub fn get_or_insert_with<F>(&mut self, stat_id: StatId, default: F) -> &mut T
    where
        F: FnOnce() -> T,
    {
        self.inner.entry(stat_id).or_insert_with(default)
    }

    #[inline]
    pub fn retain<F>(&mut self, mut f: F)
    where
        F: FnMut(&mut T) -> bool,
    {
        self.inner.retain(|_, v| f(v))
    }

    #[inline]
    pub fn drain(&mut self) -> Drain<StatId, T> {
        self.inner.drain()
    }
}
