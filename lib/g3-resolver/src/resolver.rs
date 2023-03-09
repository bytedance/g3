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

use std::io;
use std::sync::Arc;
use std::thread::JoinHandle;

use log::warn;
use tokio::sync::mpsc;

use super::ResolverStats;
use crate::config::ResolverConfig;
use crate::handle::ResolverHandle;
use crate::message::{ResolveDriverRequest, ResolverCommand};
use crate::runtime::ResolverRuntime;

pub struct ResolverBuilder {
    resolver_config: ResolverConfig,
    thread_name: Option<String>,
}

pub struct Resolver {
    config: ResolverConfig,
    stats: Arc<ResolverStats>,
    thread_handle: Option<JoinHandle<()>>,
    req_sender: mpsc::UnboundedSender<ResolveDriverRequest>,
    ctl_sender: mpsc::UnboundedSender<ResolverCommand>,
}

impl ResolverBuilder {
    pub fn new(config: ResolverConfig) -> Self {
        ResolverBuilder {
            resolver_config: config,
            thread_name: None,
        }
    }

    pub fn thread_name(&mut self, name: String) {
        self.thread_name = Some(name);
    }

    pub fn build(mut self) -> io::Result<Resolver> {
        let (req_sender, req_receiver) = mpsc::unbounded_channel();
        let (ctl_sender, ctl_receiver) = mpsc::unbounded_channel();

        let mut thread_builder = std::thread::Builder::new();
        if let Some(name) = self.thread_name.take() {
            thread_builder = thread_builder.name(name);
        }
        let config = self.resolver_config.clone();
        let stats = Arc::new(ResolverStats::default());
        let stats_a = Arc::clone(&stats);
        let thread_handle = thread_builder.spawn(move || {
            let basic_rt = tokio::runtime::Builder::new_current_thread()
                .enable_all()
                .build()
                .unwrap();
            basic_rt.block_on(async move {
                let resolver_name = config.name.to_owned();
                let resolver_runtime =
                    ResolverRuntime::new(config, req_receiver, ctl_receiver, stats_a);
                if let Err(e) = resolver_runtime.await {
                    warn!("resolver {resolver_name} runtime exited with error: {e}",);
                }
            });
        })?;

        Ok(Resolver {
            config: self.resolver_config,
            stats,
            thread_handle: Some(thread_handle),
            req_sender,
            ctl_sender,
        })
    }
}

impl Resolver {
    pub fn get_stats(&self) -> Arc<ResolverStats> {
        Arc::clone(&self.stats)
    }

    pub fn get_handle(&self) -> ResolverHandle {
        ResolverHandle::new(self.req_sender.clone())
    }

    pub fn get_config(&self) -> ResolverConfig {
        self.config.clone()
    }

    pub fn update_config(&mut self, config: ResolverConfig) -> io::Result<()> {
        if self.config.eq(&config) {
            return Ok(());
        }

        if self.config.name.ne(&config.name) {
            return Err(io::Error::new(
                io::ErrorKind::InvalidInput,
                "resolver name mismatch",
            ));
        }

        match self
            .ctl_sender
            .send(ResolverCommand::Update(Box::new(config.clone())))
        {
            Ok(_) => {
                self.config = config;
                Ok(())
            }
            Err(e) => Err(io::Error::new(io::ErrorKind::Other, e)),
        }
    }

    fn stop(&self) {
        let _ = self.ctl_sender.send(ResolverCommand::Quit);
    }

    pub async fn shutdown(&mut self) {
        if let Some(join) = self.thread_handle.take() {
            tokio::time::sleep(self.config.runtime.graceful_stop_wait).await;

            self.stop();
            let resolver_name = self.config.name.to_owned();
            if let Err(e) = tokio::task::spawn_blocking(move || {
                let thread_id = join.thread().id();
                if let Err(e) = join.join() {
                    warn!(
                        "error while waiting thread {thread_id:?} for resolver {resolver_name}: {e:?}",
                    );
                }
            })
            .await
            {
                warn!(
                    "error while waiting shutdown task for resolver {}: {e:?}",
                    self.config.name
                );
            }
        }
    }
}

impl Drop for Resolver {
    fn drop(&mut self) {
        if let Some(join) = self.thread_handle.take() {
            self.stop();
            let thread_id = join.thread().id();
            if let Err(e) = join.join() {
                warn!(
                    "error while waiting thread {thread_id:?} for resolver {}: {e:?}",
                    self.config.name
                );
            }
        }
    }
}
