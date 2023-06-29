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

use std::cell::RefCell;
use std::sync::atomic::{AtomicUsize, Ordering};

use rand::Rng;
use tokio::runtime::Handle;

use g3_runtime::unaided::WorkersGuard;

#[derive(Clone)]
pub struct WorkerHandle {
    pub handle: Handle,
    pub id: usize,
}

static mut WORKER_HANDLERS: Vec<WorkerHandle> = Vec::new();

static LISTEN_RR_INDEX: AtomicUsize = AtomicUsize::new(0);
thread_local! {
    static WORKER_RR_INDEX: RefCell<Option<usize>> = const { RefCell::new(None) };
}

pub async fn spawn_workers() -> anyhow::Result<Option<WorkersGuard>> {
    if let Some(config) = crate::runtime::config::get_worker_config() {
        let guard = config
            .start(&|id, handle| unsafe { WORKER_HANDLERS.push(WorkerHandle { handle, id }) })
            .await?;
        Ok(Some(guard))
    } else {
        Ok(None)
    }
}

#[inline]
fn handles() -> &'static [WorkerHandle] {
    unsafe { &WORKER_HANDLERS }
}

pub fn worker_count() -> usize {
    handles().len()
}

pub fn select_handle() -> Option<WorkerHandle> {
    let handles = handles();

    match handles.len() {
        0 => None,
        1 => Some(handles[0].clone()),
        n => WORKER_RR_INDEX.with(|cell| {
            let mut id = cell.borrow().map(|v| v + 1).unwrap_or_else(|| {
                let mut rng = rand::thread_rng();
                rng.gen_range(0..n)
            });
            if id >= n {
                id = 0;
            }
            let handle = unsafe { handles.get_unchecked(id).clone() };
            *cell.borrow_mut() = Some(id);
            Some(handle)
        }),
    }
}

pub fn select_listen_handle() -> Option<WorkerHandle> {
    let handles = handles();

    match handles.len() {
        0 => None,
        1 => Some(handles[0].clone()),
        len => {
            let mut prev = LISTEN_RR_INDEX.load(Ordering::Acquire);
            let max = len - 1;
            loop {
                let next = if prev >= max { 0 } else { prev + 1 };
                match LISTEN_RR_INDEX.compare_exchange(
                    prev,
                    next,
                    Ordering::AcqRel,
                    Ordering::Acquire,
                ) {
                    Ok(p) => return Some(handles[p].clone()),
                    Err(n) => prev = n,
                }
            }
        }
    }
}
