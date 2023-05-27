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

use slog::{slog_o, Drain};
use slog_scope::GlobalLoggerGuard;

use g3_types::log::AsyncLogConfig;

use crate::opts::DaemonArgs;

const PROCESS_LOG_THREAD_NAME: &str = "log-process";

pub fn setup(args: &DaemonArgs) -> Result<GlobalLoggerGuard, log::SetLoggerError> {
    let async_conf = AsyncLogConfig::with_name(PROCESS_LOG_THREAD_NAME);
    let logger = if args.with_systemd {
        cfg_if::cfg_if! {
            if #[cfg(target_os = "linux")] {
                let drain = g3_journal::new_async_logger(&async_conf, true);
                slog::Logger::root(drain.fuse(), slog_o!())
            } else {
                unreachable!()
            }
        }
    } else if args.daemon_mode {
        let drain = g3_syslog::SyslogBuilder::with_ident(args.process_name.to_string())
            .start_async(&async_conf);
        slog::Logger::root(drain.fuse(), slog_o!())
    } else {
        let drain = g3_stdlog::new_async_logger(&async_conf, true);
        slog::Logger::root(drain.fuse(), slog_o!())
    };

    let scope_guard = slog_scope::set_global_logger(logger);

    let log_level = match args.verbose_level {
        0 => log::Level::Warn,
        1 => log::Level::Info,
        2 => log::Level::Debug,
        _ => log::Level::Trace,
    };

    slog_stdlog::init_with_level(log_level)?;
    Ok(scope_guard)
}
