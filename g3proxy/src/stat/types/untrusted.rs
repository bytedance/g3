/*
 * SPDX-License-Identifier: Apache-2.0
 * Copyright 2023-2025 ByteDance and/or its affiliates.
 */

#[derive(Default)]
pub(crate) struct UntrustedTaskStatsSnapshot {
    pub(crate) task_total: u64,
    pub(crate) task_alive: i32,
    pub(crate) in_bytes: u64,
}
