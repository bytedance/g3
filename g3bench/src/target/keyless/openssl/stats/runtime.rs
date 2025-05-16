/*
 * SPDX-License-Identifier: Apache-2.0
 * Copyright 2023-2025 ByteDance and/or its affiliates.
 */

use std::sync::atomic::{AtomicI64, AtomicU64, Ordering};
use std::time::Duration;

use g3_statsd_client::StatsdClient;

use crate::target::BenchRuntimeStats;

#[derive(Default)]
pub(crate) struct KeylessRuntimeStats {
    task_total: AtomicU64,
    task_alive: AtomicI64,
    task_passed: AtomicU64,
    task_failed: AtomicU64,
}

impl KeylessRuntimeStats {
    pub(crate) fn add_task_total(&self) {
        self.task_total.fetch_add(1, Ordering::Relaxed);
    }

    pub(crate) fn inc_task_alive(&self) {
        self.task_alive.fetch_add(1, Ordering::Relaxed);
    }

    pub(crate) fn dec_task_alive(&self) {
        self.task_alive.fetch_sub(1, Ordering::Relaxed);
    }

    pub(crate) fn add_task_passed(&self) {
        self.task_passed.fetch_add(1, Ordering::Relaxed);
    }

    pub(crate) fn add_task_failed(&self) {
        self.task_failed.fetch_add(1, Ordering::Relaxed);
    }
}

impl BenchRuntimeStats for KeylessRuntimeStats {
    fn emit(&self, client: &mut StatsdClient) {
        macro_rules! emit_count {
            ($field:ident, $name:literal) => {
                let v = self.$field.swap(0, Ordering::Relaxed);
                client.count(concat!("keyless.", $name), v).send();
            };
        }

        let task_alive = self.task_alive.load(Ordering::Relaxed);
        client.gauge("keyless.task.alive", task_alive).send();

        emit_count!(task_total, "task.total");
        emit_count!(task_passed, "task.passed");
        emit_count!(task_failed, "task.failed");
    }

    fn summary(&self, _total_time: Duration) {}
}
