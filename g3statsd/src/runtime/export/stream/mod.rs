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

use std::io;
use std::time::Duration;

use chrono::{DateTime, Utc};
use log::{debug, warn};
use tokio::io::{AsyncRead, AsyncReadExt, AsyncWrite, AsyncWriteExt};
use tokio::sync::mpsc;

use crate::types::MetricRecord;

mod config;
pub(crate) use config::StreamExportConfig;

pub(crate) trait StreamExport {
    fn serialize(&self, time: DateTime<Utc>, record: &MetricRecord, buf: &mut Vec<u8>);
}

struct StreamExportRuntime<T: StreamExport> {
    config: StreamExportConfig,
    formatter: T,
    receiver: mpsc::Receiver<(DateTime<Utc>, MetricRecord)>,

    write_buf: Vec<u8>,
    quit: bool,
}

impl<T> StreamExportRuntime<T>
where
    T: StreamExport,
{
    fn new(
        config: StreamExportConfig,
        formatter: T,
        receiver: mpsc::Receiver<(DateTime<Utc>, MetricRecord)>,
    ) -> Self {
        StreamExportRuntime {
            config,
            formatter,
            receiver,
            write_buf: Vec::with_capacity(2048),
            quit: false,
        }
    }

    async fn into_running(mut self) {
        loop {
            match self.config.connect().await {
                Ok(stream) => self.run_with_stream(stream).await,
                Err(wait) => self.drop_wait(wait).await,
            }
            if self.quit {
                break;
            }
        }
    }

    async fn drop_wait(&mut self, wait: Duration) {
        if tokio::time::timeout(wait, async {
            while self.receiver.recv().await.is_some() {
                // TODO add metrics
            }
        })
        .await
        .is_ok()
        {
            self.quit = true
        }
    }

    async fn run_with_stream<S>(&mut self, mut stream: S)
    where
        S: AsyncRead + AsyncWrite + Unpin,
    {
        const BATCH_SIZE: usize = 16;

        let mut buf = Vec::with_capacity(BATCH_SIZE);
        let mut read_buf = [0u8; BATCH_SIZE];

        loop {
            buf.clear();

            tokio::select! {
                biased;

                r =  stream.read(&mut read_buf) => {
                    match r {
                        Ok(_) => {
                            debug!("exporter {}: connection closed by peer", self.config.exporter);
                        }
                        Err(e) => {
                            debug!("exporter {}: connection closed by peer: {e}", self.config.exporter);
                        }
                    }
                    break;
                }
                n = self.receiver.recv_many(&mut buf, BATCH_SIZE) => {
                    if n == 0 {
                        self.quit = true;
                        break;
                    }

                    if let Err(e) = self.send_msg(&mut stream, &buf).await {
                        warn!("exporter {}: failed to send records: {e}", self.config.exporter);
                        break;
                    }
                }
            }
        }
    }

    async fn send_msg<W>(
        &mut self,
        writer: &mut W,
        records: &[(DateTime<Utc>, MetricRecord)],
    ) -> io::Result<()>
    where
        W: AsyncWrite + Unpin,
    {
        self.write_buf.clear();

        for r in records {
            self.formatter.serialize(r.0, &r.1, &mut self.write_buf);
        }

        writer.write_all(&self.write_buf).await
    }
}
