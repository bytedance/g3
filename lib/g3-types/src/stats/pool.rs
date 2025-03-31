/*
 * Copyright 2024 ByteDance and/or its affiliates.
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

use std::sync::Arc;
use std::sync::atomic::{AtomicIsize, AtomicU64, Ordering};

#[derive(Default)]
pub struct ConnectionPoolStats {
    total_connection: AtomicU64,
    alive_connection: AtomicIsize,
}

impl ConnectionPoolStats {
    pub fn add_connection(self: &Arc<Self>) -> ConnectionPoolAliveConnectionGuard {
        self.total_connection.fetch_add(1, Ordering::Relaxed);
        self.alive_connection.fetch_add(1, Ordering::Relaxed);
        ConnectionPoolAliveConnectionGuard {
            stats: self.clone(),
        }
    }

    pub fn alive_count(&self) -> usize {
        self.alive_connection
            .load(Ordering::Relaxed)
            .try_into()
            .unwrap_or_default()
    }
}

pub struct ConnectionPoolAliveConnectionGuard {
    stats: Arc<ConnectionPoolStats>,
}

impl Drop for ConnectionPoolAliveConnectionGuard {
    fn drop(&mut self) {
        self.stats.alive_connection.fetch_sub(1, Ordering::Relaxed);
    }
}
