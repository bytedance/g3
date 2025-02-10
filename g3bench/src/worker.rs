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
