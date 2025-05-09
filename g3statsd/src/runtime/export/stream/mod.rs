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

use log::{debug, warn};
use tokio::io::{AsyncRead, AsyncReadExt, AsyncWrite};
use tokio::sync::mpsc;

use g3_io_ext::LimitedWriteExt;

mod config;
pub(crate) use config::StreamExportConfig;

const BATCH_SIZE: usize = 16;

pub(crate) trait StreamExport {
    type Piece;

    fn serialize(&self, record: &[Self::Piece], buf: &mut Vec<u8>) -> usize;
}

pub(crate) struct StreamExportRuntime<T: StreamExport> {
    config: StreamExportConfig,
    formatter: T,
    receiver: mpsc::UnboundedReceiver<T::Piece>,

    recv_buf: Vec<T::Piece>,
    recv_handled: usize,
    write_buf: Vec<u8>,
    quit: bool,
}

impl<T> StreamExportRuntime<T>
where
    T: StreamExport,
{
    pub(crate) fn new(
        config: StreamExportConfig,
        formatter: T,
        receiver: mpsc::UnboundedReceiver<T::Piece>,
    ) -> Self {
        StreamExportRuntime {
            config,
            formatter,
            receiver,
            recv_buf: Vec::with_capacity(BATCH_SIZE),
            recv_handled: 0,
            write_buf: Vec::with_capacity(2048),
            quit: false,
        }
    }

    pub(crate) async fn into_running(mut self) {
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
        let mut read_buf = [0u8; BATCH_SIZE];

        loop {
            if self.recv_handled < self.recv_buf.len() {
                if let Err(e) = self.send_records(&mut stream).await {
                    warn!(
                        "exporter {}: failed to send records: {e:?}",
                        self.config.exporter
                    );
                    break;
                }
                continue;
            } else {
                self.recv_buf.clear();
                self.recv_handled = 0;
            }

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
                n = self.receiver.recv_many(&mut self.recv_buf, BATCH_SIZE) => {
                    if n == 0 {
                        self.quit = true;
                        break;
                    }
                }
            }
        }
    }

    async fn send_records<W>(&mut self, writer: &mut W) -> io::Result<()>
    where
        W: AsyncWrite + Unpin,
    {
        self.write_buf.clear();

        let records = &self.recv_buf[self.recv_handled..];
        let handled = self.formatter.serialize(records, &mut self.write_buf);
        if handled == 0 {
            warn!(
                "exporter {}: found too large piece when send data",
                self.config.exporter
            );
            // TODO add drop metrics
            self.recv_handled += 1;
        } else {
            self.recv_handled += handled;
        }

        writer.write_all_flush(&self.write_buf).await
    }
}
