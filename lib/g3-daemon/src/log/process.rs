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

use std::sync::OnceLock;

use log::{LevelFilter, Metadata, Record};
use slog::{Drain, Logger, slog_o};

use g3_types::log::AsyncLogConfig;

use crate::opts::DaemonArgs;

const PROCESS_LOG_THREAD_NAME: &str = "log-process";

static PROCESS_LOGGER: OnceLock<Logger> = OnceLock::new();

pub fn setup(args: &DaemonArgs) {
    let async_conf = AsyncLogConfig::with_name(PROCESS_LOG_THREAD_NAME);
    let logger = if args.with_systemd {
        cfg_if::cfg_if! {
            if #[cfg(target_os = "linux")] {
                let journal_conf = g3_journal::JournalConfig::with_ident(args.process_name).append_code_position();
                let drain = g3_journal::new_async_logger(&async_conf, journal_conf);
                Logger::root(drain.fuse(), slog_o!())
            } else {
                unreachable!()
            }
        }
    } else if args.daemon_mode {
        let drain =
            g3_syslog::SyslogBuilder::with_ident(args.process_name).start_async(&async_conf);
        Logger::root(drain.fuse(), slog_o!())
    } else {
        let drain = g3_stdlog::new_async_logger(&async_conf, true, false);
        Logger::root(drain.fuse(), slog_o!())
    };

    let _ = PROCESS_LOGGER.set(logger);

    let log_level = match args.verbose_level {
        0 => LevelFilter::Warn,
        1 => LevelFilter::Info,
        2 => LevelFilter::Debug,
        _ => LevelFilter::Trace,
    };
    log::set_max_level(log_level);
    log::set_boxed_logger(Box::new(BridgeLogger {
        level_filter: log_level,
    }))
    .unwrap();
}

struct BridgeLogger {
    level_filter: LevelFilter,
}

impl log::Log for BridgeLogger {
    fn enabled(&self, metadata: &Metadata) -> bool {
        metadata.level().to_level_filter() < self.level_filter
    }

    fn log(&self, record: &Record) {
        let Some(logger) = PROCESS_LOGGER.get() else {
            return;
        };

        let level = match record.level() {
            log::Level::Trace => slog::Level::Trace,
            log::Level::Debug => slog::Level::Debug,
            log::Level::Info => slog::Level::Info,
            log::Level::Warn => slog::Level::Warning,
            log::Level::Error => slog::Level::Error,
        };

        let location = slog::RecordLocation {
            file: record.file_static().unwrap_or("<unknown>"),
            line: record.line().unwrap_or_default(),
            column: 0,
            function: "",
            module: record.module_path_static().unwrap_or("<unknown>"),
        };

        let s = slog::RecordStatic {
            location: &location,
            level,
            tag: "",
        };

        logger.log(&slog::Record::new(&s, record.args(), slog::b!()));
    }

    fn flush(&self) {}
}
