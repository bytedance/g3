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

use std::mem;
use std::sync::Arc;
use std::time::Duration;

use async_trait::async_trait;
use log::debug;
use tokio::sync::{broadcast, mpsc};

use g3_types::net::ConnectionPoolConfig;
use g3_types::stats::ConnectionPoolStats;

use super::KeylessForwardRequest;

pub(crate) trait KeylessUpstreamConnection {
    fn run(self, idle_timeout: Duration) -> impl Future<Output = anyhow::Result<()>> + Send;
}

#[async_trait]
pub(crate) trait KeylessUpstreamConnect {
    type Connection: KeylessUpstreamConnection;
    async fn new_connection(
        &self,
        req_receiver: flume::Receiver<KeylessForwardRequest>,
        quit_notifier: broadcast::Receiver<()>,
        idle_timeout: Duration,
    ) -> anyhow::Result<Self::Connection>;
}
pub(crate) type ArcKeylessUpstreamConnect<C> =
    Arc<dyn KeylessUpstreamConnect<Connection = C> + Send + Sync>;

const CMD_CHANNEL_SIZE: usize = 16;

enum KeylessPoolCmd {
    UpdatePeers,
    CloseGraceful,
    NewConnection,
}

#[derive(Clone)]
pub(crate) struct KeylessConnectionPoolHandle {
    cmd_sender: mpsc::Sender<KeylessPoolCmd>,
}

impl KeylessConnectionPoolHandle {
    pub(crate) async fn update_peers(&self) {
        let _ = self.cmd_sender.send(KeylessPoolCmd::UpdatePeers).await;
    }

    pub(crate) async fn close_graceful(&self) {
        let _ = self.cmd_sender.send(KeylessPoolCmd::CloseGraceful).await;
    }

    pub(crate) fn request_new_connection(&self) {
        let _ = self.cmd_sender.try_send(KeylessPoolCmd::NewConnection);
    }
}

pub(crate) struct KeylessConnectionPool<C: KeylessUpstreamConnection> {
    config: ConnectionPoolConfig,
    connector: ArcKeylessUpstreamConnect<C>,
    stats: Arc<ConnectionPoolStats>,

    keyless_request_receiver: flume::Receiver<KeylessForwardRequest>,

    connection_id: u64,
    connection_close_receiver: mpsc::Receiver<u64>,
    connection_close_sender: mpsc::Sender<u64>,

    connection_quit_notifier: broadcast::Sender<()>,
    graceful_close_wait: Duration,
}

impl<C> KeylessConnectionPool<C>
where
    C: KeylessUpstreamConnection + Send + 'static,
{
    fn new(
        config: ConnectionPoolConfig,
        connector: ArcKeylessUpstreamConnect<C>,
        keyless_request_receiver: flume::Receiver<KeylessForwardRequest>,
        graceful_close_wait: Duration,
    ) -> Self {
        let (connection_close_sender, connection_close_receiver) = mpsc::channel(1);
        let connection_quit_notifier = broadcast::Sender::new(1);
        KeylessConnectionPool {
            config,
            connector,
            stats: Arc::new(ConnectionPoolStats::default()),
            keyless_request_receiver,
            connection_id: 0,
            connection_close_receiver,
            connection_close_sender,
            connection_quit_notifier,
            graceful_close_wait,
        }
    }

    pub(crate) fn spawn(
        config: ConnectionPoolConfig,
        connector: ArcKeylessUpstreamConnect<C>,
        keyless_request_receiver: flume::Receiver<KeylessForwardRequest>,
        graceful_close_wait: Duration,
    ) -> KeylessConnectionPoolHandle {
        let pool = KeylessConnectionPool::new(
            config,
            connector,
            keyless_request_receiver,
            graceful_close_wait,
        );
        let (cmd_sender, cmd_receiver) = mpsc::channel(CMD_CHANNEL_SIZE);
        tokio::spawn(async move {
            pool.into_running(cmd_receiver).await;
        });
        KeylessConnectionPoolHandle { cmd_sender }
    }

    async fn into_running(mut self, mut cmd_receiver: mpsc::Receiver<KeylessPoolCmd>) {
        let mut connection_check_interval = tokio::time::interval(self.config.check_interval());

        loop {
            tokio::select! {
                r = cmd_receiver.recv() => {
                    let Some(cmd) = r else {
                        break;
                    };

                    match cmd {
                        KeylessPoolCmd::UpdatePeers => {
                            let new_quit_notifier = broadcast::Sender::new(1);
                            let old_quit_notifier = mem::replace(&mut self.connection_quit_notifier, new_quit_notifier);
                            let graceful_close_wait = self.graceful_close_wait;
                            tokio::spawn(async move {
                                tokio::time::sleep(graceful_close_wait).await;
                                let _ = old_quit_notifier.send(());
                            });
                            let target = self.stats.alive_count().max(self.config.min_idle_count());
                            self.check_create_connection(0, target);
                        }
                        KeylessPoolCmd::CloseGraceful => {
                            tokio::time::sleep(self.graceful_close_wait).await;
                            let _ = self.connection_quit_notifier.send(());
                            break;
                        }
                        KeylessPoolCmd::NewConnection => {
                            if self.stats.alive_count() < self.config.max_idle_count() {
                                self.create_connection();
                            }
                        }
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
        let keyless_request_receiver = self.keyless_request_receiver.clone();
        let connection_close_sender = self.connection_close_sender.clone();
        let connection_quit_notifier = self.connection_quit_notifier.subscribe();
        let pool_stats = self.stats.clone();
        let idle_timeout = self.config.idle_timeout();
        tokio::spawn(async move {
            pool_stats.add_connection();
            match connector
                .new_connection(
                    keyless_request_receiver,
                    connection_quit_notifier,
                    idle_timeout,
                )
                .await
            {
                Ok(connection) => {
                    if let Err(e) = connection.run(idle_timeout).await {
                        debug!("connection closed with error: {e}");
                    }
                    pool_stats.del_connection();
                    let _ = connection_close_sender.try_send(connection_id);
                }
                Err(e) => {
                    debug!("failed to create new connection: {e}");
                    pool_stats.del_connection();
                }
            }
        });
    }
}
