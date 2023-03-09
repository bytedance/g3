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

use std::collections::VecDeque;
use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::Arc;

use anyhow::anyhow;
use flume::Receiver;
use log::warn;
use tokio::io::{AsyncRead, AsyncReadExt, AsyncWrite, AsyncWriteExt};
use tokio::net::TcpStream;
use tokio_openssl::SslStream;

use g3_types::log::{AsyncLogConfig, AsyncLogger, LogStats};

mod config;
pub use config::FluentdClientConfig;

mod handshake;

#[macro_use]
mod macros;

mod format;
pub use format::FluentdFormatter;

pub fn new_async_logger(
    async_conf: &AsyncLogConfig,
    fluent_conf: &Arc<FluentdClientConfig>,
    tag_name: String,
) -> AsyncLogger<Vec<u8>, FluentdFormatter> {
    let (sender, receiver) = flume::bounded::<Vec<u8>>(async_conf.channel_capacity);

    let stats = Arc::new(LogStats::default());

    for i in 0..async_conf.thread_number {
        let io_thread = AsyncIoThread {
            config: Arc::clone(fluent_conf),
            receiver: receiver.clone(),
            stats: Arc::clone(&stats),
            retry_queue: VecDeque::with_capacity(fluent_conf.retry_queue_len),
        };

        let _detached_thread = std::thread::Builder::new()
            .name(format!("{}#{i}", async_conf.thread_name))
            .spawn(move || {
                let rt = tokio::runtime::Builder::new_current_thread()
                    .enable_all()
                    .build()
                    .unwrap();
                rt.block_on(io_thread.run_to_end());
            });
    }

    AsyncLogger::new(sender, FluentdFormatter::new(tag_name), stats)
}

enum FluentdConnection {
    Tcp(TcpStream),
    Tls(SslStream<TcpStream>),
}

struct AsyncIoThread {
    config: Arc<FluentdClientConfig>,
    receiver: Receiver<Vec<u8>>,
    stats: Arc<LogStats>,
    retry_queue: VecDeque<Vec<u8>>,
}

impl AsyncIoThread {
    async fn run_to_end(mut self) {
        loop {
            match tokio::time::timeout(self.config.connect_timeout, self.config.new_connection())
                .await
            {
                Ok(Ok(connection)) => {
                    let r = match connection {
                        FluentdConnection::Tcp(tcp_stream) => {
                            self.run_with_connection(tcp_stream).await
                        }
                        FluentdConnection::Tls(tls_stream) => {
                            self.run_with_connection(tls_stream).await
                        }
                    };
                    match r {
                        Ok(_) => break,
                        Err(e) => warn!("lost connection to fluentd: {e:?}"),
                    }
                }
                Ok(Err(e)) => {
                    warn!("failed to connect to fluentd server: {e:?}");
                    match self.run_without_connection().await {
                        Ok(_) => break,
                        Err(e) => warn!("{e:?}"),
                    }
                }
                Err(_) => {
                    warn!("timed out to connect to fluentd server");
                    match self.run_without_connection().await {
                        Ok(_) => break,
                        Err(e) => warn!("{e:?}"),
                    }
                }
            }
        }
    }

    async fn run_without_connection(&mut self) -> anyhow::Result<()> {
        let drop_count = Arc::new(AtomicUsize::new(0));
        let drop_count_i = drop_count.clone();
        match tokio::time::timeout(self.config.connect_delay, async {
            while let Ok(data) = self.receiver.recv_async().await {
                if self.push_to_retry(data).is_some() {
                    drop_count_i.fetch_add(1, Ordering::Relaxed);
                }
            }
        })
        .await
        {
            Ok(_) => Ok(()),
            Err(_) => Err(anyhow!(
                "will retry connect again. {} logs dropped during this period",
                drop_count.load(Ordering::Relaxed)
            )),
        }
    }

    async fn run_with_connection<T>(&mut self, mut connection: T) -> anyhow::Result<()>
    where
        T: AsyncRead + AsyncWrite + Unpin,
    {
        let mut read_buf = [0u8; 8];
        let mut flush_interval = tokio::time::interval(self.config.flush_interval);
        // skip flush_interval.tick().await;

        while let Some(data) = self.retry_queue.pop_front() {
            match tokio::time::timeout(
                self.config.write_timeout,
                connection.write_all(data.as_slice()),
            )
            .await
            {
                Ok(Ok(_)) => {}
                Ok(Err(e)) => {
                    self.retry_queue.push_front(data);
                    return Err(anyhow!("write event failed: {e:?}"));
                }
                Err(_) => {
                    // drop directly on write timeout
                    self.stats.drop.add_peer_unreachable();
                }
            }
        }

        loop {
            tokio::select! {
                r = self.receiver.recv_async() => {
                    match r {
                        Ok(data) => {
                            match tokio::time::timeout(self.config.write_timeout, connection.write_all(data.as_slice())).await {
                                Ok(Ok(_)) => {}
                                Ok(Err(e)) => {
                                    self.push_to_retry(data);
                                    return Err(anyhow!("write event failed: {e:?}"));
                                }
                                Err(_) => {
                                    // drop directly on write timeout
                                    self.stats.drop.add_peer_unreachable();
                                }
                            }
                        }
                        Err(_) => return Ok(()),
                    }
                }
                r = connection.read(&mut read_buf) => {
                    return match r {
                        Ok(0) => Err(anyhow!("connection closed by server")),
                        Ok(_) => Err(anyhow!("unexpected data received, will close this connection")),
                        Err(e) => Err(anyhow!("connection closed: {e:?}")),
                    };
                }
                _ = flush_interval.tick() => {
                    connection.flush().await.map_err(|e| anyhow!("flush data failed: {e:?}"))?;
                }
            }
        }
    }

    fn push_to_retry(&mut self, data: Vec<u8>) -> Option<Vec<u8>> {
        self.retry_queue.push_back(data);
        if self.retry_queue.len() > self.config.retry_queue_len {
            self.stats.drop.add_peer_unreachable();
            self.retry_queue.pop_front()
        } else {
            None
        }
    }
}
