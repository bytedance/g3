/*
 * SPDX-License-Identifier: Apache-2.0
 * Copyright 2024-2025 ByteDance and/or its affiliates.
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
