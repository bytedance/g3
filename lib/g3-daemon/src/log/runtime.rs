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

use slog::{slog_o, Logger, OwnedKV, SendSyncRefUnwindSafeKV};

use g3_types::log::AsyncLogConfig;

use super::{LogConfig, LogConfigDriver, LoggerStats, ReportLogIoError};

pub fn create_shared_logger(
    logger_name: String,
    daemon_group: &'static str,
    log_type: &'static str,
    config: &LogConfig,
) -> Logger {
    let common_values = slog_o!(
        "daemon_name" => daemon_group,
        "log_type" => log_type,
        "pid" => std::process::id(),
    );
    create_logger(config, logger_name, log_type, common_values)
}

pub fn create_logger<T>(
    config: &LogConfig,
    logger_name: String,
    log_type: &'static str,
    common_values: OwnedKV<T>,
) -> Logger
where
    T: SendSyncRefUnwindSafeKV + 'static,
{
    match config.driver.clone() {
        LogConfigDriver::Discard => {
            let drain = slog::Discard {};
            Logger::root(drain, common_values)
        }
        LogConfigDriver::Journal => {
            let async_conf = AsyncLogConfig {
                channel_capacity: config.async_channel_size,
                thread_number: config.async_thread_number,
                thread_name: logger_name.clone(),
            };
            let drain = g3_journal::new_async_logger(&async_conf, false);
            let logger_stats = LoggerStats::new(&logger_name, drain.get_stats());
            super::registry::add(logger_name.clone(), Arc::new(logger_stats));
            let drain = ReportLogIoError::new(drain, &logger_name, config.io_err_sampling_mask);
            Logger::root(drain, common_values)
        }
        LogConfigDriver::Syslog(builder) => {
            let async_conf = AsyncLogConfig {
                channel_capacity: config.async_channel_size,
                thread_number: config.async_thread_number,
                thread_name: logger_name.clone(),
            };
            let drain = builder.start_async(&async_conf);
            let logger_stats = LoggerStats::new(&logger_name, drain.get_stats());
            super::registry::add(logger_name.clone(), Arc::new(logger_stats));
            let drain = ReportLogIoError::new(drain, &logger_name, config.io_err_sampling_mask);
            Logger::root(drain, common_values)
        }
        LogConfigDriver::Fluentd(fluentd_conf) => {
            let async_conf = AsyncLogConfig {
                channel_capacity: config.async_channel_size,
                thread_number: config.async_thread_number,
                thread_name: logger_name.clone(),
            };
            let drain = g3_fluentd::new_async_logger(
                &async_conf,
                &fluentd_conf,
                format!("{}.{log_type}", config.program_name),
            );
            let logger_stats = LoggerStats::new(&logger_name, drain.get_stats());
            super::registry::add(logger_name.clone(), Arc::new(logger_stats));
            let drain = ReportLogIoError::new(drain, &logger_name, config.io_err_sampling_mask);
            Logger::root(drain, common_values)
        }
    }
}
