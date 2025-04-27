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
use std::num::NonZeroUsize;
use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::mpsc;

use anyhow::anyhow;
use log::{error, trace, warn};
use tokio::runtime::{Handle, Runtime};
use tokio::sync::watch;

use g3_compat::CpuAffinity;

#[cfg(feature = "yaml")]
mod yaml;

pub struct MvWorkersGuard {
    _rt_list: Vec<Runtime>,
}

pub struct CvWorkersGuard {
    _close_sender: watch::Sender<()>,
}

pub enum WorkersGuard {
    VariantC(CvWorkersGuard),
    VariantM(MvWorkersGuard),
}

pub struct UnaidedRuntimeConfig {
    thread_number_total: NonZeroUsize,
    thread_number_per_rt: NonZeroUsize,
    thread_stack_size: Option<usize>,
    sched_affinity: HashMap<usize, CpuAffinity>,
    max_io_events_per_tick: Option<usize>,
    #[cfg(feature = "openssl-async-job")]
    openssl_async_job_init_size: usize,
    #[cfg(feature = "openssl-async-job")]
    openssl_async_job_max_size: usize,
}

impl Default for UnaidedRuntimeConfig {
    fn default() -> Self {
        Self::new()
    }
}

impl UnaidedRuntimeConfig {
    pub fn new() -> Self {
        let target_thread_number =
            std::thread::available_parallelism().unwrap_or(NonZeroUsize::MIN);
        UnaidedRuntimeConfig {
            thread_number_total: target_thread_number,
            thread_number_per_rt: NonZeroUsize::MIN,
            thread_stack_size: None,
            sched_affinity: HashMap::new(),
            max_io_events_per_tick: None,
            #[cfg(feature = "openssl-async-job")]
            openssl_async_job_init_size: 0,
            #[cfg(feature = "openssl-async-job")]
            openssl_async_job_max_size: 0,
        }
    }

    pub fn set_thread_number_per_rt(&mut self, num: NonZeroUsize) {
        self.thread_number_per_rt = num;
    }

    pub fn set_thread_number_total(&mut self, num: NonZeroUsize) {
        self.thread_number_total = num;
    }

    pub fn set_thread_stack_size(&mut self, size: usize) {
        self.thread_stack_size = Some(size);
    }

    pub fn set_sched_affinity(&mut self, id: usize, cpus: CpuAffinity) {
        self.sched_affinity.insert(id, cpus);
    }

    #[cfg(any(
        target_os = "linux",
        target_os = "android",
        target_os = "freebsd",
        target_os = "dragonfly",
        target_os = "netbsd",
        windows,
    ))]
    pub fn auto_set_sched_affinity(&mut self) -> anyhow::Result<()> {
        use anyhow::Context;

        let mut found_env_config = false;
        for i in 0..self.thread_number_total.get() / self.thread_number_per_rt.get() {
            let var_name = format!("WORKER_{i}_CPU_LIST");
            if let Some(os_s) = std::env::var_os(&var_name) {
                let Some(s) = os_s.to_str() else {
                    return Err(anyhow!("failed to decode env var {var_name}"));
                };
                let mut cpu = CpuAffinity::default();
                cpu.parse_add(s)
                    .context(format!("invalid CPU ID list value for env var {var_name}"))?;
                self.sched_affinity.insert(i, cpu);
                found_env_config = true;
            }
        }
        if found_env_config {
            return Ok(());
        }

        if self.thread_number_per_rt.get() != 1 {
            return Err(anyhow!(
                "unable to set CPU affinity for multi thread worker runtime"
            ));
        }
        let n = self.thread_number_total.get();
        for i in 0..n {
            let mut cpu = CpuAffinity::default();
            cpu.add_id(i)
                .map_err(|e| anyhow!("unable to build cpu set for cpu {i}: {e}"))?;
            self.sched_affinity.insert(i, cpu);
        }
        Ok(())
    }

    pub fn set_max_io_events_per_tick(&mut self, capacity: usize) {
        self.max_io_events_per_tick = Some(capacity);
    }

    #[cfg(feature = "openssl-async-job")]
    pub fn set_openssl_async_job_init_size(&mut self, size: usize) {
        if g3_openssl::async_job::async_is_capable() {
            self.openssl_async_job_init_size = size;
        } else if size > 0 {
            warn!("openssl async job is not supported");
        }
    }

    #[cfg(feature = "openssl-async-job")]
    pub fn set_openssl_async_job_max_size(&mut self, size: usize) {
        if g3_openssl::async_job::async_is_capable() {
            self.openssl_async_job_max_size = size;
        } else if size > 0 {
            warn!("openssl async job is not supported");
        }
    }

    pub fn check(&mut self) -> anyhow::Result<()> {
        let threads_per_rt = self.thread_number_per_rt.get();
        if self.thread_number_total.get() % threads_per_rt != 0 {
            return Err(anyhow!(
                "total thread number {} is not dividable by per-runtime thread number {}",
                self.thread_number_total,
                threads_per_rt
            ));
        }
        Ok(())
    }

    fn start_variant_m<F>(
        &self,
        recv_handle: F,
        rt_num: usize,
        rt_thread_num: usize,
    ) -> anyhow::Result<WorkersGuard>
    where
        F: Fn(usize, Handle, Option<CpuAffinity>),
    {
        let mut rt_list = Vec::with_capacity(rt_num);
        for i in 0..rt_num {
            let mut rt_builder = tokio::runtime::Builder::new_multi_thread();
            rt_builder.worker_threads(rt_thread_num);
            if let Some(stack_size) = self.thread_stack_size {
                rt_builder.thread_stack_size(stack_size);
            }
            if let Some(n) = self.max_io_events_per_tick {
                rt_builder.max_io_events_per_tick(n);
            }
            rt_builder.enable_all();

            rt_builder.thread_name_fn(move || {
                static ATOMIC_ID: AtomicUsize = AtomicUsize::new(0);
                let id = ATOMIC_ID.fetch_add(1, Ordering::SeqCst);
                format!("worker-{i}#{id}")
            });

            if let Some(cpu_affinity) = self.sched_affinity.get(&i).cloned() {
                rt_builder.on_thread_start(move || {
                    if let Err(e) = cpu_affinity.apply_to_local_thread() {
                        warn!("failed to set sched affinity for worker thread {i}: {e}");
                    }
                });
            }

            match rt_builder.build() {
                Ok(rt) => {
                    let cpu_affinity = self.sched_affinity.get(&i).cloned();
                    recv_handle(i, rt.handle().clone(), cpu_affinity);
                    rt_list.push(rt);
                }
                Err(e) => return Err(anyhow!("failed to create tokio worker runtime {i}: {e}")),
            }
        }

        Ok(WorkersGuard::VariantM(MvWorkersGuard { _rt_list: rt_list }))
    }

    fn start_variant_c<F>(&self, recv_handle: F, thread_num: usize) -> anyhow::Result<WorkersGuard>
    where
        F: Fn(usize, Handle, Option<CpuAffinity>),
    {
        let (close_w, _close_r) = watch::channel(());

        for i in 0..thread_num {
            let mut close_r = close_w.subscribe();
            let (sender, receiver) = mpsc::sync_channel(1);

            let mut thread_builder = std::thread::Builder::new().name(format!("worker#{i}"));

            if let Some(stack_size) = self.thread_stack_size {
                thread_builder = thread_builder.stack_size(stack_size);
            }

            let cpu_set = self.sched_affinity.get(&i).cloned();
            let max_io_events_per_tick = self.max_io_events_per_tick;
            #[cfg(feature = "openssl-async-job")]
            let openssl_async_job_init_size = self.openssl_async_job_init_size;
            #[cfg(feature = "openssl-async-job")]
            let openssl_async_job_max_size = self.openssl_async_job_max_size;

            thread_builder
                .spawn(move || {
                    trace!("started worker thread #{i}");

                    if let Some(set) = cpu_set {
                        if let Err(e) = set.apply_to_local_thread() {
                            warn!("failed to set sched affinity for worker thread {i}: {e}");
                        }
                    }

                    let mut builder = tokio::runtime::Builder::new_current_thread();
                    builder.enable_all();
                    if let Some(n) = max_io_events_per_tick {
                        builder.max_io_events_per_tick(n);
                    }

                    #[cfg(feature = "openssl-async-job")]
                    builder.on_thread_start(move || {
                        if let Err(e) = g3_openssl::async_job::async_thread_init(
                            openssl_async_job_max_size,
                            openssl_async_job_init_size,
                        ) {
                            warn!(
                                "failed to init ({}, {}) openssl async jobs: {e}",
                                openssl_async_job_max_size, openssl_async_job_init_size
                            );
                        }
                    });
                    #[cfg(feature = "openssl-async-job")]
                    builder.on_thread_stop(g3_openssl::async_job::async_thread_cleanup);

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
                            error!("failed to create tokio runtime on worker thread {i}: {e}");
                        }
                    }
                    trace!("stopped worker thread #{i}");
                })
                .map_err(|e| anyhow!("failed to spawn worker thread {i}: {e}"))?;

            match receiver.recv() {
                Ok(handle) => {
                    let cpu_affinity = self.sched_affinity.get(&i).cloned();
                    recv_handle(i, handle, cpu_affinity)
                }
                Err(_) => {
                    return Err(anyhow!(
                        "no tokio runtime handler got from worker thread {i}",
                    ));
                }
            }
        }

        Ok(WorkersGuard::VariantC(CvWorkersGuard {
            _close_sender: close_w,
        }))
    }

    pub fn start<F>(&self, recv_handle: F) -> anyhow::Result<WorkersGuard>
    where
        F: Fn(usize, Handle, Option<CpuAffinity>),
    {
        let threads_per_rt = self.thread_number_per_rt.get();
        if threads_per_rt == 1 {
            self.start_variant_c(recv_handle, self.thread_number_total.get())
        } else {
            self.start_variant_m(
                recv_handle,
                self.thread_number_total.get() / threads_per_rt,
                threads_per_rt,
            )
        }
    }
}
