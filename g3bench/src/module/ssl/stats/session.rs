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

use std::sync::atomic::{AtomicU64, Ordering};

#[derive(Default)]
pub(crate) struct SslSessionStats {
    total: AtomicU64,
    reused: AtomicU64,
}

impl SslSessionStats {
    #[inline]
    pub(crate) fn add_total(&self) {
        self.total.fetch_add(1, Ordering::Relaxed);
    }

    #[inline]
    pub(crate) fn add_reused(&self) {
        self.reused.fetch_add(1, Ordering::Relaxed);
    }

    pub(crate) fn summary(&self, prefix: &'static str) {
        let total = self.total.load(Ordering::Relaxed);
        if total == 0 {
            return;
        }

        let session_reused = self.reused.load(Ordering::Relaxed);
        println!("# {prefix} Session");
        println!("Reused Count: {session_reused}");
        println!(
            "Reuse Ratio: {:.2}%",
            (session_reused as f64 / total as f64) * 100.0
        );
    }
}
