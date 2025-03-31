/*
 * Copyright 2024 ByteDance and/or its affiliates.
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

use std::sync::Arc;

use tokio::sync::mpsc;

use g3_types::net::ConnectionPoolConfig;
use g3_types::stats::ConnectionPoolStats;

use super::{StreamDetourConnector, StreamDetourRequest};

const CMD_CHANNEL_SIZE: usize = 16;

enum PoolCommand {
    NewConnection,
}

pub(super) struct StreamDetourPoolHandle {
    cmd_sender: mpsc::Sender<PoolCommand>,
}

impl StreamDetourPoolHandle {
    pub(super) fn request_new_connection(&self) {
        let _ = self.cmd_sender.try_send(PoolCommand::NewConnection);
    }
}

pub(super) struct StreamDetourPool {
    config: ConnectionPoolConfig,
    connector: Arc<StreamDetourConnector>,
    stats: Arc<ConnectionPoolStats>,

    client_req_receiver: flume::Receiver<StreamDetourRequest>,

    connection_id: u64,
    connection_close_receiver: mpsc::Receiver<u64>,
    connection_close_sender: mpsc::Sender<u64>,
}

impl StreamDetourPool {
    fn new(
        config: ConnectionPoolConfig,
        client_req_receiver: flume::Receiver<StreamDetourRequest>,
        connector: Arc<StreamDetourConnector>,
    ) -> Self {
        let (connection_close_sender, connection_close_receiver) = mpsc::channel(1);
        StreamDetourPool {
            config,
            connector,
            stats: Arc::new(ConnectionPoolStats::default()),
            client_req_receiver,
            connection_id: 0,
            connection_close_receiver,
            connection_close_sender,
        }
    }

    pub(super) fn spawn(
        config: ConnectionPoolConfig,
        client_cmd_receiver: flume::Receiver<StreamDetourRequest>,
        connector: Arc<StreamDetourConnector>,
    ) -> StreamDetourPoolHandle {
        let pool = StreamDetourPool::new(config, client_cmd_receiver, connector);
        let (cmd_sender, cmd_receiver) = mpsc::channel(CMD_CHANNEL_SIZE);
        tokio::spawn(async move {
            pool.into_running(cmd_receiver).await;
        });
        StreamDetourPoolHandle { cmd_sender }
    }

    async fn into_running(mut self, mut cmd_receiver: mpsc::Receiver<PoolCommand>) {
        let mut connection_check_interval = tokio::time::interval(self.config.check_interval());

        loop {
            tokio::select! {
                r = cmd_receiver.recv() => {
                    let Some(cmd) = r else {
                        break;
                    };

                    match cmd {
                        PoolCommand::NewConnection => self.create_connection(),
                    }
                }
                _ = self.connection_close_receiver.recv() => {
                    self.check_create_connection(self.stats.alive_count(), self.config.min_idle_count());
                }
                _ = connection_check_interval.tick() => {
                    self.check_create_connection(self.stats.alive_count(), self.config.min_idle_count());
                }
            }
        }
    }

    fn check_create_connection(&mut self, alive: usize, target: usize) {
        if alive < target {
            for _ in alive..target {
                self.create_connection();
            }
        }
    }

    fn create_connection(&mut self) {
        self.connection_id += 1;
        let connector = self.connector.clone();
        let connection_id = self.connection_id;
        let client_req_receiver = self.client_req_receiver.clone();
        let connection_close_sender = self.connection_close_sender.clone();
        let pool_stats = self.stats.clone();
        let idle_timeout = self.config.idle_timeout();
        tokio::spawn(async move {
            let alive_guard = pool_stats.add_connection();
            connector
                .run_new_connection(client_req_receiver, idle_timeout)
                .await;
            drop(alive_guard);
            let _ = connection_close_sender.try_send(connection_id);
        });
    }
}
