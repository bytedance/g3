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
use libsystemd::logging::Priority;

use g3_types::log::{AsyncLogConfig, AsyncLogger, LogStats};

#[macro_use]
mod macros;

mod format;
pub use format::JournalFormatter;

pub struct JournalValue {
    priority: Priority,
    msg: String,
    vars: Vec<(String, String)>,
}

pub fn new_async_logger(
    async_conf: &AsyncLogConfig,
    append_code_position: bool,
) -> AsyncLogger<JournalValue, JournalFormatter> {
    let (sender, receiver) = flume::bounded::<JournalValue>(async_conf.channel_capacity);

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

    AsyncLogger::new(sender, JournalFormatter::new(append_code_position), stats)
}

struct AsyncIoThread {
    receiver: Receiver<JournalValue>,
    stats: Arc<LogStats>,
}

impl AsyncIoThread {
    fn run_to_end(self) {
        while let Ok(v) = self.receiver.recv() {
            if libsystemd::logging::journal_send(v.priority, &v.msg, v.vars.into_iter()).is_err() {
                self.stats.drop.add_peer_unreachable();
            }
        }
    }
}
