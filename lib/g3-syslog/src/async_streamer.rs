/*
 * SPDX-License-Identifier: Apache-2.0
 * Copyright 2023-2025 ByteDance and/or its affiliates.
 */

use std::cell::RefCell;
use std::sync::Arc;
use std::time::{Duration, Instant};

use flume::{Receiver, Sender, TrySendError};
use log::warn;
use slog::{Drain, OwnedKVList, Record};

use g3_types::log::{AsyncLogConfig, LogStats};

use super::{BoxSyslogFormatter, SyslogBackendBuilder, SyslogHeader};
use crate::backend::MAX_BATCH_SIZE;
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
                stats: stats.clone(),
                recv_buf: Vec::with_capacity(MAX_BATCH_SIZE),
                backend_container: None,
                backend_failed_instant: Instant::now(),
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
        TL_BUF.with_borrow_mut(|buf| {
            self.stats.io.add_total();
            buf.clear();

            match self
                .formatter
                .format_slog(buf, &self.header, record, logger_values)
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
    recv_buf: Vec<String>,
    backend_container: Option<SyslogBackend>,
    backend_failed_instant: Instant,
}

impl AsyncIoThread {
    fn run_to_end(mut self) {
        while let Ok(s) = self.receiver.recv() {
            self.recv_buf.push(s);
            self.recv_send_all();
        }
    }

    fn recv_send_all(&mut self) {
        while self.recv_buf.len() < MAX_BATCH_SIZE {
            let Ok(s) = self.receiver.try_recv() else {
                break;
            };
            self.recv_buf.push(s);
        }

        let mut already_sent = 0;
        while already_sent < self.recv_buf.len() {
            let to_sent = &self.recv_buf[already_sent..];
            if let Some(backend) = self.backend_container.take() {
                match backend.write_many(to_sent) {
                    Ok(0) => {
                        self.stats.drop.add_peer_unreachable_n(to_sent.len());
                        self.recv_buf.clear();
                        warn!("sent zero msg to syslog backend, will reconnect later");
                        self.backend_failed_instant = Instant::now();
                        return;
                    }
                    Ok(n) => {
                        self.stats.io.add_passed_n(n);
                        let size = to_sent.iter().take(n).map(|b| b.len()).sum();
                        self.stats.io.add_size(size);
                        already_sent += n;
                        self.backend_container = Some(backend);
                    }
                    Err(e) => {
                        self.stats.drop.add_peer_unreachable_n(to_sent.len());
                        self.recv_buf.clear();
                        warn!("failed to send msg to syslog backend: {e}, will reconnect later");
                        self.backend_failed_instant = Instant::now();
                        return;
                    }
                }
            } else {
                if self.backend_failed_instant.elapsed() > Duration::from_secs(4) {
                    // hard coded 4s for a minimal reconnect interval
                    match self.backend_builder.build() {
                        Ok(backend) => {
                            self.backend_container = Some(backend);
                            continue;
                        }
                        Err(e) => {
                            warn!("failed to build syslog backend: {e}, will reconnect later");
                            self.backend_failed_instant = Instant::now();
                        }
                    }
                }
                self.stats.drop.add_peer_unreachable_n(to_sent.len());
                self.recv_buf.clear();
                return;
            }
        }
        self.recv_buf.clear();
    }
}
