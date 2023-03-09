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

use flume::{Sender, TrySendError};
use slog::{Drain, OwnedKVList, Record};

use super::LogStats;

#[derive(Clone, Debug)]
pub struct AsyncLogConfig {
    pub channel_capacity: usize,
    pub thread_number: usize,
    pub thread_name: String,
}

impl AsyncLogConfig {
    pub fn with_name(thread_name: &str) -> Self {
        AsyncLogConfig {
            channel_capacity: 1024,
            thread_number: 1,
            thread_name: thread_name.to_string(),
        }
    }
}

impl Default for AsyncLogConfig {
    fn default() -> Self {
        AsyncLogConfig::with_name("log-async")
    }
}

pub trait AsyncLogFormatter<T> {
    fn format_slog(&self, record: &Record, logger_values: &OwnedKVList) -> Result<T, slog::Error>;
}

pub struct AsyncLogger<T, F>
where
    F: AsyncLogFormatter<T>,
{
    sender: Sender<T>,
    formatter: F,
    stats: Arc<LogStats>,
}

impl<T, F> AsyncLogger<T, F>
where
    F: AsyncLogFormatter<T>,
{
    pub fn new(sender: Sender<T>, formatter: F, stats: Arc<LogStats>) -> Self {
        AsyncLogger {
            sender,
            formatter,
            stats,
        }
    }

    pub fn get_stats(&self) -> Arc<LogStats> {
        Arc::clone(&self.stats)
    }
}

impl<T, F> Drain for AsyncLogger<T, F>
where
    F: AsyncLogFormatter<T>,
{
    type Ok = ();
    type Err = slog::Error;

    fn log(&self, record: &Record, logger_values: &OwnedKVList) -> Result<(), slog::Error> {
        self.stats.io.add_total();

        match self.formatter.format_slog(record, logger_values) {
            Ok(v) => {
                match self.sender.try_send(v) {
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
    }
}
