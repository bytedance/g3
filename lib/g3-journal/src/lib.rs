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

use std::sync::Arc;

use flume::Receiver;

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
    let (sender, receiver) = flume::bounded::<Vec<u8>>(async_conf.channel_capacity);

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
    receiver: Receiver<Vec<u8>>,
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
