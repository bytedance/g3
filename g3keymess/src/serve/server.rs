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

use slog::Logger;
use tokio::net::TcpStream;
use tokio::sync::{broadcast, Semaphore};

use g3_daemon::listen::ListenStats;
use g3_daemon::server::ServerQuitPolicy;
use g3_types::metrics::MetricsName;

use super::{
    KeyServerRuntime, KeyServerStats, KeylessTask, KeylessTaskContext, ServerReloadCommand,
};
use crate::config::server::KeyServerConfig;

pub(crate) struct KeyServer {
    config: Arc<KeyServerConfig>,
    server_stats: Arc<KeyServerStats>,
    listen_stats: Arc<ListenStats>,
    quit_policy: Arc<ServerQuitPolicy>,
    reload_sender: broadcast::Sender<ServerReloadCommand>,
    concurrency_limit: Option<Arc<Semaphore>>,
    task_logger: Logger,
    request_logger: Logger,
}

impl KeyServer {
    fn new(
        config: KeyServerConfig,
        server_stats: Arc<KeyServerStats>,
        listen_stats: Arc<ListenStats>,
        concurrency_limit: Option<Arc<Semaphore>>,
    ) -> Self {
        let (reload_sender, _reload_receiver) = broadcast::channel(16);

        let task_logger = config.get_task_logger();
        let request_logger = config.get_request_logger();

        KeyServer {
            config: Arc::new(config),
            server_stats,
            listen_stats,
            quit_policy: Arc::new(ServerQuitPolicy::default()),
            reload_sender,
            concurrency_limit,
            task_logger,
            request_logger,
        }
    }

    pub(crate) fn prepare_initial(config: KeyServerConfig) -> KeyServer {
        let server_stats = KeyServerStats::new(config.name());
        let listen_stats = ListenStats::new(config.name());
        let concurrency_limit = if config.concurrency_limit > 0 {
            Some(Arc::new(Semaphore::new(config.concurrency_limit)))
        } else {
            None
        };
        KeyServer::new(
            config,
            Arc::new(server_stats),
            Arc::new(listen_stats),
            concurrency_limit,
        )
    }

    fn prepare_reload(&self, config: KeyServerConfig) -> KeyServer {
        let server_stats = self.server_stats.clone();
        let listen_stats = self.listen_stats.clone();
        let concurrency_limit = if config.concurrency_limit > 0 {
            Some(Arc::new(Semaphore::new(config.concurrency_limit)))
        } else {
            None
        };
        KeyServer::new(config, server_stats, listen_stats, concurrency_limit)
    }

    #[inline]
    pub(crate) fn name(&self) -> &MetricsName {
        self.config.name()
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

    #[inline]
    pub(super) fn get_listen_stats(&self) -> Arc<ListenStats> {
        self.listen_stats.clone()
    }

    #[allow(unused)]
    pub(super) fn get_server_stats(&self) -> Arc<KeyServerStats> {
        self.server_stats.clone()
    }

    pub(super) fn start_runtime(&self, server: &Arc<KeyServer>) -> anyhow::Result<()> {
        KeyServerRuntime::new(server).into_running(&self.config.listen, &self.reload_sender)
    }

    pub(super) fn abort_runtime(&self) {
        let _ = self.reload_sender.send(ServerReloadCommand::QuitRuntime);
        self.server_stats.set_offline();
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
            peer_addr,
            local_addr,
            task_logger: self.task_logger.clone(),
            request_logger: self.request_logger.clone(),
            reload_notifier: self.reload_sender.subscribe(),
            concurrency_limit: self.concurrency_limit.clone(),
        };

        let (r, w) = stream.into_split();
        let task = KeylessTask::new(ctx);
        if self.config.multiplex_queue_depth > 1 {
            task.into_multiplex_running(r, w).await
        } else {
            task.into_simplex_running(r, w).await
        }
    }
}
