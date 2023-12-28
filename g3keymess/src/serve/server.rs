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

use std::net::SocketAddr;
use std::sync::Arc;

use arc_swap::ArcSwap;
use slog::Logger;
use tokio::net::TcpStream;
use tokio::sync::{broadcast, Semaphore};

use g3_daemon::listen::ListenStats;
use g3_daemon::server::ServerQuitPolicy;
use g3_types::metrics::{MetricsName, MetricsTagName, MetricsTagValue, StaticMetricsTags};

use super::{
    KeyServerDurationRecorder, KeyServerDurationStats, KeyServerRuntime, KeyServerStats,
    KeylessTask, KeylessTaskContext, ServerReloadCommand,
};
use crate::config::server::KeyServerConfig;

pub(crate) struct KeyServer {
    config: Arc<KeyServerConfig>,
    server_stats: Arc<KeyServerStats>,
    listen_stats: Arc<ListenStats>,
    duration_recorder: KeyServerDurationRecorder,
    duration_stats: Arc<KeyServerDurationStats>,
    quit_policy: Arc<ServerQuitPolicy>,
    reload_sender: broadcast::Sender<ServerReloadCommand>,
    concurrency_limit: Option<Arc<Semaphore>>,
    task_logger: Logger,
    request_logger: Logger,
    dynamic_metrics_tags: Arc<ArcSwap<StaticMetricsTags>>,
}

impl KeyServer {
    fn new(
        config: KeyServerConfig,
        server_stats: Arc<KeyServerStats>,
        listen_stats: Arc<ListenStats>,
        duration_recorder: KeyServerDurationRecorder,
        duration_stats: Arc<KeyServerDurationStats>,
        concurrency_limit: Option<Arc<Semaphore>>,
        dynamic_metrics_tags: Arc<ArcSwap<StaticMetricsTags>>,
    ) -> Self {
        let reload_sender = broadcast::Sender::new(16);

        let task_logger = config.get_task_logger();
        let request_logger = config.get_request_logger();

        // always update extra metrics tags
        let dynamic_tags = dynamic_metrics_tags.load();
        let dynamic_tags = dynamic_tags.as_ref().clone();
        if let Some(conf) = config.extra_metrics_tags.clone() {
            let mut extra = (*conf).clone();
            extra.extend(dynamic_tags);
            let extra = Arc::new(extra);
            server_stats.set_extra_tags(Some(extra.clone()));
            duration_stats.set_extra_tags(Some(extra));
        } else if !dynamic_tags.is_empty() {
            let extra = Arc::new(dynamic_tags);
            server_stats.set_extra_tags(Some(extra.clone()));
            duration_stats.set_extra_tags(Some(extra));
        }

        KeyServer {
            config: Arc::new(config),
            server_stats,
            listen_stats,
            duration_recorder,
            duration_stats,
            quit_policy: Arc::new(ServerQuitPolicy::default()),
            reload_sender,
            concurrency_limit,
            task_logger,
            request_logger,
            dynamic_metrics_tags,
        }
    }

    pub(crate) fn prepare_initial(config: KeyServerConfig) -> KeyServer {
        let server_stats = KeyServerStats::new(config.name());
        let listen_stats = ListenStats::new(config.name());
        let (duration_recorder, duration_stats) =
            KeyServerDurationRecorder::new(config.name(), &config.duration_stats);
        let concurrency_limit = if config.concurrency_limit > 0 {
            Some(Arc::new(Semaphore::new(config.concurrency_limit)))
        } else {
            None
        };
        KeyServer::new(
            config,
            Arc::new(server_stats),
            Arc::new(listen_stats),
            duration_recorder,
            duration_stats,
            concurrency_limit,
            Arc::new(ArcSwap::new(Default::default())),
        )
    }

    fn prepare_reload(&self, config: KeyServerConfig) -> KeyServer {
        let concurrency_limit = if config.concurrency_limit > 0 {
            Some(Arc::new(Semaphore::new(config.concurrency_limit)))
        } else {
            None
        };
        let (duration_recorder, duration_stats) =
            if self.config.duration_stats != config.duration_stats {
                KeyServerDurationRecorder::new(config.name(), &config.duration_stats)
            } else {
                (self.duration_recorder.clone(), self.duration_stats.clone())
            };
        KeyServer::new(
            config,
            self.server_stats.clone(),
            self.listen_stats.clone(),
            duration_recorder,
            duration_stats,
            concurrency_limit,
            self.dynamic_metrics_tags.clone(),
        )
    }

    #[inline]
    pub(crate) fn name(&self) -> &MetricsName {
        self.config.name()
    }

    pub(crate) fn listen_addr(&self) -> SocketAddr {
        self.config.listen.address()
    }

    pub(super) fn alive_count(&self) -> i32 {
        self.server_stats.get_alive_count()
    }

    #[inline]
    pub(super) fn quit_policy(&self) -> &Arc<ServerQuitPolicy> {
        &self.quit_policy
    }

    #[inline]
    pub(super) fn clone_config(&self) -> Arc<KeyServerConfig> {
        self.config.clone()
    }

    pub(super) fn reload_with_new_notifier(&self, config: KeyServerConfig) -> KeyServer {
        self.prepare_reload(config)
    }

    pub(crate) fn add_dynamic_metrics_tag(&self, name: MetricsTagName, value: MetricsTagValue) {
        let dynamic_tags = self.dynamic_metrics_tags.load();
        let mut dynamic_tags = dynamic_tags.as_ref().clone();
        dynamic_tags.insert(name, value);
        self.dynamic_metrics_tags
            .store(Arc::new(dynamic_tags.clone()));

        match self.server_stats.load_extra_tags() {
            Some(extra) => {
                let mut extra = (*extra).clone();
                extra.extend(dynamic_tags);
                self.server_stats.set_extra_tags(Some(Arc::new(extra)))
            }
            None => self
                .server_stats
                .set_extra_tags(Some(Arc::new(dynamic_tags))),
        }
    }

    #[inline]
    pub(crate) fn get_listen_stats(&self) -> Arc<ListenStats> {
        self.listen_stats.clone()
    }

    #[inline]
    pub(crate) fn get_server_stats(&self) -> Arc<KeyServerStats> {
        self.server_stats.clone()
    }

    #[inline]
    pub(crate) fn get_duration_stats(&self) -> Arc<KeyServerDurationStats> {
        self.duration_stats.clone()
    }

    pub(super) fn start_runtime(&self, server: &Arc<KeyServer>) -> anyhow::Result<()> {
        KeyServerRuntime::new(server)
            .into_running(&self.config.listen, &self.reload_sender)
            .map(|_| {
                server.server_stats.set_online();
                server.duration_stats.set_online()
            })
    }

    pub(super) fn abort_runtime(&self) {
        let _ = self.reload_sender.send(ServerReloadCommand::QuitRuntime);
        self.server_stats.set_offline();
        self.duration_stats.set_offline();
    }

    pub(super) async fn run_tcp_task(
        &self,
        stream: TcpStream,
        peer_addr: SocketAddr,
        local_addr: SocketAddr,
    ) {
        let ctx = KeylessTaskContext {
            server_config: self.config.clone(),
            server_stats: self.server_stats.clone(),
            duration_recorder: self.duration_recorder.clone(),
            peer_addr,
            local_addr,
            task_logger: self.task_logger.clone(),
            request_logger: self.request_logger.clone(),
            reload_notifier: self.reload_sender.subscribe(),
            concurrency_limit: self.concurrency_limit.clone(),
        };

        let (r, w) = stream.into_split();
        let task = KeylessTask::new(ctx);

        #[cfg(feature = "openssl-async-job")]
        if self.config.multiplex_queue_depth > 1 {
            task.into_multiplex_running(r, w).await
        } else {
            task.into_simplex_running(r, w).await
        }

        #[cfg(not(feature = "openssl-async-job"))]
        task.into_simplex_running(r, w).await
    }
}
