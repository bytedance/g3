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

use std::collections::HashMap;
use std::num::NonZeroUsize;

use anyhow::anyhow;
use log::{error, trace};
use nix::sched::CpuSet;
use nix::unistd::Pid;
use tokio::runtime::Handle;
use tokio::sync::{oneshot, watch};

pub struct WorkersGuard(watch::Sender<()>);

pub struct UnaidedRuntimeConfig {
    thread_number: Option<NonZeroUsize>,
    thread_stack_size: Option<usize>,
    sched_affinity: HashMap<usize, CpuSet>,
    max_io_events_per_tick: Option<usize>,
}

impl Default for UnaidedRuntimeConfig {
    fn default() -> Self {
        Self::new()
    }
}

impl UnaidedRuntimeConfig {
    pub fn new() -> Self {
        UnaidedRuntimeConfig {
            thread_number: None,
            thread_stack_size: None,
            sched_affinity: HashMap::new(),
            max_io_events_per_tick: None,
        }
    }

    pub fn set_thread_number(&mut self, num: usize) {
        if let Ok(n) = NonZeroUsize::try_from(num) {
            self.thread_number = Some(n);
        } else {
            self.thread_number = None;
        }
    }

    pub fn set_thread_stack_size(&mut self, size: usize) {
        self.thread_stack_size = Some(size);
    }

    pub fn set_sched_affinity(&mut self, id: usize, cpus: CpuSet) {
        self.sched_affinity.insert(id, cpus);
    }

    pub fn set_mapped_sched_affinity(&mut self) -> anyhow::Result<()> {
        let n = self
            .thread_number
            .map(|v| v.get())
            .unwrap_or_else(num_cpus::get);
        for i in 0..n {
            let mut cpu = CpuSet::new();
            cpu.set(i)
                .map_err(|e| anyhow!("unable to build cpu set for cpu {i}: {}", e.desc()))?;
            self.sched_affinity.insert(i, cpu);
        }
        Ok(())
    }

    pub fn set_max_io_events_per_tick(&mut self, capacity: usize) {
        self.max_io_events_per_tick = Some(capacity);
    }

    pub async fn start<F>(&self, recv_handle: &F) -> anyhow::Result<WorkersGuard>
    where
        F: Fn(usize, Handle),
    {
        let n = self
            .thread_number
            .map(|n| n.get())
            .unwrap_or_else(num_cpus::get);

        let (close_w, _close_r) = watch::channel(());

        for i in 0..n {
            let mut close_r = close_w.subscribe();
            let (sender, receiver) = oneshot::channel();

            let mut thread_builder = std::thread::Builder::new().name(format!("worker#{i}"));

            if let Some(thread_stack_size) = self.thread_stack_size {
                thread_builder = thread_builder.stack_size(thread_stack_size);
            }

            let cpu_set = self.sched_affinity.get(&i).cloned();
            let max_io_events_per_tick = self.max_io_events_per_tick;

            thread_builder
                .spawn(move || {
                    trace!("started worker thread #{i}");

                    if let Some(set) = cpu_set {
                        if let Err(e) = nix::sched::sched_setaffinity(Pid::from_raw(0), &set) {
                            error!(
                                "failed to set sched affinity for worker thread {i}: {}",
                                e.desc()
                            );
                        }
                    }

                    let mut builder = tokio::runtime::Builder::new_current_thread();
                    builder.enable_all();
                    if let Some(n) = max_io_events_per_tick {
                        builder.max_io_events_per_tick(n);
                    }

                    match builder.build() {
                        Ok(rt) => {
                            rt.block_on(async move {
                                let handle = Handle::current();
                                if sender.send(handle).is_ok() {
                                    let _ = close_r.changed().await;
                                }
                            });
                        }
                        Err(e) => {
                            error!("failed to create tokio runtime on worker thread {i}: {e}",);
                        }
                    }
                    trace!("stopped worker thread #{}", i);
                })
                .map_err(|e| anyhow!("failed to spawn worker thread {i}: {e}"))?;

            match receiver.await {
                Ok(handle) => recv_handle(i, handle),
                Err(_) => {
                    return Err(anyhow!(
                        "no tokio runtime handler got from worker thread {i}",
                    ))
                }
            }
        }

        Ok(WorkersGuard(close_w))
    }
}
