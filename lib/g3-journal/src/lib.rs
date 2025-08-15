/*
 * SPDX-License-Identifier: Apache-2.0
 * Copyright 2023-2025 ByteDance and/or its affiliates.
 */

use std::sync::Arc;

use g3_types::log::{AsyncLogConfig, AsyncLogger, LogStats};

#[macro_use]
mod macros;

mod io;

mod format;
pub use format::JournalFormatter;

#[derive(Clone, Copy)]
pub struct JournalConfig {
    ident: &'static str,
    append_code_position: bool,
}

impl JournalConfig {
    pub fn with_ident(ident: &'static str) -> Self {
        JournalConfig {
            ident,
            append_code_position: false,
        }
    }

    pub fn append_code_position(mut self) -> Self {
        self.append_code_position = true;
        self
    }
}

pub fn new_async_logger(
    async_conf: &AsyncLogConfig,
    journal_conf: JournalConfig,
) -> AsyncLogger<Vec<u8>, JournalFormatter> {
    let (sender, receiver) = kanal::bounded::<Vec<u8>>(async_conf.channel_capacity);

    let stats = Arc::new(LogStats::default());

    for i in 0..async_conf.thread_number {
        let io_thread = AsyncIoThread {
            receiver: receiver.clone(),
            stats: Arc::clone(&stats),
        };

        let _detached_thread = std::thread::Builder::new()
            .name(format!("{}#{i}", async_conf.thread_name))
            .spawn(move || {
                io_thread.run_to_end();
            });
    }

    AsyncLogger::new(sender, JournalFormatter::new(journal_conf), stats)
}

struct AsyncIoThread {
    receiver: kanal::Receiver<Vec<u8>>,
    stats: Arc<LogStats>,
}

impl AsyncIoThread {
    fn run_to_end(self) {
        while let Ok(v) = self.receiver.recv() {
            match io::journal_send(&v) {
                Ok(_) => {
                    self.stats.io.add_passed();
                    self.stats.io.add_size(v.len());
                }
                Err(_) => self.stats.drop.add_peer_unreachable(),
            }
        }
    }
}
