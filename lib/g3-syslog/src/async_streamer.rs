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

use std::cell::RefCell;
use std::io::{self, Write};
use std::sync::Arc;
use std::time::{Duration, Instant};

use flume::{Receiver, Sender, TrySendError};
use log::warn;
use slog::{Drain, OwnedKVList, Record};

use g3_types::log::{AsyncLogConfig, LogStats};

use super::{BoxSyslogFormatter, SyslogBackendBuilder, SyslogHeader};
use crate::backend::SyslogBackend;

thread_local! {
    static TL_BUF: RefCell<Vec<u8>> = RefCell::new(Vec::with_capacity(1024))
}

pub struct AsyncSyslogStreamer {
    header: SyslogHeader,
    sender: Sender<String>,
    formatter: BoxSyslogFormatter,
    stats: Arc<LogStats>,
}

impl AsyncSyslogStreamer {
    pub(super) fn new(
        config: &AsyncLogConfig,
        header: SyslogHeader,
        formatter: BoxSyslogFormatter,
        backend_builder: &SyslogBackendBuilder,
    ) -> Self {
        let (sender, receiver) = flume::bounded::<String>(config.channel_capacity);

        let stats = Arc::new(LogStats::default());

        for i in 0..config.thread_number {
            let io_thread = AsyncIoThread {
                receiver: receiver.clone(),
                backend_builder: backend_builder.clone(),
                stats: Arc::clone(&stats),
            };

            let _detached_thread = std::thread::Builder::new()
                .name(format!("{}#{i}", config.thread_name))
                .spawn(move || {
                    io_thread.run_to_end();
                });
        }

        AsyncSyslogStreamer {
            header,
            sender,
            formatter,
            stats,
        }
    }

    pub fn get_stats(&self) -> Arc<LogStats> {
        Arc::clone(&self.stats)
    }
}

impl Drain for AsyncSyslogStreamer {
    type Ok = ();
    type Err = slog::Error;

    fn log(&self, record: &Record, logger_values: &OwnedKVList) -> Result<(), slog::Error> {
        TL_BUF.with(|buf| {
            self.stats.io.add_total();

            let mut buf = buf.borrow_mut();
            buf.clear();

            match self
                .formatter
                .format_slog(&mut buf, &self.header, record, logger_values)
            {
                Ok(_) => {
                    let s = unsafe { String::from_utf8_unchecked(buf.clone()) };
                    match self.sender.try_send(s) {
                        Ok(_) => {}
                        Err(TrySendError::Full(_)) => self.stats.drop.add_channel_overflow(),
                        Err(TrySendError::Disconnected(_)) => self.stats.drop.add_channel_closed(),
                    }

                    Ok(())
                }
                Err(e) => {
                    self.stats.drop.add_format_failed();
                    Err(e)
                }
            }
        })
    }
}

struct AsyncIoThread {
    receiver: Receiver<String>,
    backend_builder: SyslogBackendBuilder,
    stats: Arc<LogStats>,
}

impl AsyncIoThread {
    fn run_to_end(self) {
        let mut backend_container: Option<SyslogBackend> = self.build_backend();
        let mut failed_instant = Instant::now();
        while let Ok(s) = self.receiver.recv() {
            if let Some(mut backend) = backend_container.take() {
                if self.send_data(s, &mut backend).is_err() {
                    self.stats.drop.add_peer_unreachable();
                    if backend.need_reconnect() {
                        failed_instant = Instant::now();
                    } else {
                        backend_container = Some(backend);
                    }
                } else {
                    backend_container = Some(backend);
                }
            } else {
                if failed_instant.elapsed() > Duration::from_secs(4) {
                    // hard coded 4s for a minimal reconnect interval
                    if let Some(mut backend) = self.build_backend() {
                        if self.send_data(s, &mut backend).is_ok() {
                            backend_container = Some(backend);
                            continue;
                        }
                    }
                }
                self.stats.drop.add_peer_unreachable();
            }
        }
    }

    fn build_backend(&self) -> Option<SyslogBackend> {
        match self.backend_builder.build() {
            Ok(backend) => Some(backend),
            Err(e) => {
                warn!("failed to build syslog backend: {e:?}");
                None
            }
        }
    }

    fn send_data(&self, data: String, backend: &mut SyslogBackend) -> io::Result<()> {
        let size = data.len();
        backend.write_all(data.as_bytes())?;
        backend.flush()?;
        self.stats.io.add_passed();
        self.stats.io.add_size(size);
        Ok(())
    }
}
