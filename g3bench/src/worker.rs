/*
 * SPDX-License-Identifier: Apache-2.0
 * Copyright 2023-2025 ByteDance and/or its affiliates.
 */

use tokio::runtime::Handle;

use g3_runtime::unaided::{UnaidedRuntimeConfig, WorkersGuard};
use g3_types::sync::GlobalInit;

static WORKER_HANDLERS: GlobalInit<Vec<Handle>> = GlobalInit::new(Vec::new());

pub fn spawn_workers(config: &UnaidedRuntimeConfig) -> anyhow::Result<WorkersGuard> {
    let guard = config.start(|_, handle, _| WORKER_HANDLERS.with_mut(|vec| vec.push(handle)))?;
    Ok(guard)
}

pub(super) fn select_handle(concurrency_index: usize) -> Option<Handle> {
    let handlers = WORKER_HANDLERS.as_ref();
    match handlers.len() {
        0 => None,
        1 => Some(handlers[0].clone()),
        n => {
            let handle = unsafe { handlers.get_unchecked(concurrency_index % n) };
            Some(handle.clone())
        }
    }
}
