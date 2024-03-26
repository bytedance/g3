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

use std::future::Future;
use std::sync::atomic::{AtomicIsize, AtomicU64, Ordering};
use std::sync::Arc;
use std::time::Duration;

use async_trait::async_trait;
use log::debug;
use tokio::sync::{broadcast, mpsc};

use super::KeylessForwardRequest;

pub(crate) trait KeylessUpstreamConnection {
    fn run(self) -> impl Future<Output = anyhow::Result<()>> + Send;
}

#[async_trait]
pub(crate) trait KeylessUpstreamConnect {
    type Connection: KeylessUpstreamConnection;
    async fn new_connection(
        &self,
        req_receiver: flume::Receiver<KeylessForwardRequest>,
        quit_notifier: broadcast::Receiver<Duration>,
    ) -> anyhow::Result<Self::Connection>;
}
pub(crate) type ArcKeylessUpstreamConnect<C> =
    Arc<dyn KeylessUpstreamConnect<Connection = C> + Send + Sync>;

enum KeylessPoolCmd {
    UpdatePeers,
    Close,
}

#[derive(Clone)]
pub(crate) struct KeylessConnectionPoolHandle {
    cmd_sender: mpsc::Sender<KeylessPoolCmd>,
}

impl KeylessConnectionPoolHandle {
    pub(crate) async fn update_peers(&self) {
        let _ = self.cmd_sender.send(KeylessPoolCmd::UpdatePeers).await;
    }

    pub(crate) async fn close(&self) {
        let _ = self.cmd_sender.send(KeylessPoolCmd::Close).await;
    }
}

#[derive(Default)]
struct PoolStats {
    total_connection: AtomicU64,
    alive_connection: AtomicIsize,
}

impl PoolStats {
    fn add_connection(&self) {
        self.total_connection.fetch_add(1, Ordering::Relaxed);
        self.alive_connection.fetch_add(1, Ordering::Relaxed);
    }

    fn del_connection(&self) {
        self.total_connection.fetch_sub(1, Ordering::Relaxed);
        self.alive_connection.fetch_sub(1, Ordering::Relaxed);
    }

    fn alive_count(&self) -> usize {
        self.alive_connection
            .load(Ordering::Relaxed)
            .try_into()
            .unwrap_or_default()
    }
}

pub(crate) struct KeylessConnectionPool<C: KeylessUpstreamConnection> {
    connector: ArcKeylessUpstreamConnect<C>,
    idle_connection_min: usize,
    idle_connection_max: usize,
    stats: Arc<PoolStats>,

    keyless_request_receiver: flume::Receiver<KeylessForwardRequest>,

    connection_id: u64,
    connection_close_receiver: mpsc::Receiver<u64>,
    connection_close_sender: mpsc::Sender<u64>,

    connection_quit_notifier: broadcast::Sender<Duration>,
    graceful_close_wait: Duration,
}

impl<C> KeylessConnectionPool<C>
where
    C: KeylessUpstreamConnection + Send + 'static,
{
    fn new(
        connector: ArcKeylessUpstreamConnect<C>,
        idle_connection_min: usize,
        idle_connection_max: usize,
        keyless_request_receiver: flume::Receiver<KeylessForwardRequest>,
        graceful_close_wait: Duration,
    ) -> Self {
        let (connection_close_sender, connection_close_receiver) = mpsc::channel(1);
        let connection_quit_notifier = broadcast::Sender::new(idle_connection_max);
        KeylessConnectionPool {
            connector,
            idle_connection_min,
            idle_connection_max,
            stats: Arc::new(PoolStats::default()),
            keyless_request_receiver,
            connection_id: 0,
            connection_close_receiver,
            connection_close_sender,
            connection_quit_notifier,
            graceful_close_wait,
        }
    }

    pub(crate) fn spawn(
        connector: ArcKeylessUpstreamConnect<C>,
        idle_connection_min: usize,
        idle_connection_max: usize,
        keyless_request_receiver: flume::Receiver<KeylessForwardRequest>,
        graceful_close_wait: Duration,
    ) -> KeylessConnectionPoolHandle {
        let pool = KeylessConnectionPool::new(
            connector,
            idle_connection_min,
            idle_connection_max,
            keyless_request_receiver,
            graceful_close_wait,
        );
        let (cmd_sender, cmd_receiver) = mpsc::channel(16);
        tokio::spawn(async move {
            pool.into_running(cmd_receiver).await;
        });
        KeylessConnectionPoolHandle { cmd_sender }
    }

    async fn into_running(mut self, mut cmd_receiver: mpsc::Receiver<KeylessPoolCmd>) {
        loop {
            tokio::select! {
                r = cmd_receiver.recv() => {
                    let Some(cmd) = r else {
                        break;
                    };

                    match cmd {
                        KeylessPoolCmd::UpdatePeers => {
                            let _ = self.connection_quit_notifier.send(self.graceful_close_wait);
                            self.connection_quit_notifier = broadcast::Sender::new(self.idle_connection_max);
                            self.check_create_connection(0, self.stats.alive_count());
                        }
                        KeylessPoolCmd::Close => {
                            let _ = self.connection_quit_notifier.send(self.graceful_close_wait);
                            break;
                        }
                    }
                }
                _ = self.connection_close_receiver.recv() => {
                    self.check_create_connection(self.stats.alive_count(), self.idle_connection_min);
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
        pool_stats.add_connection();
        tokio::spawn(async move {
            match connector
                .new_connection(keyless_request_receiver, connection_quit_notifier)
                .await
            {
                Ok(connection) => {
                    if let Err(e) = connection.run().await {
                        debug!("connection closed with error: {e}");
                    }
                }
                Err(e) => {
                    debug!("failed to create new connection: {e}");
                }
            }
            pool_stats.del_connection();
            let _ = connection_close_sender.try_send(connection_id);
        });
    }
}
