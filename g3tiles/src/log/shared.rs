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

use std::collections::{hash_map::Entry, HashMap};
use std::sync::Mutex;

use once_cell::sync::Lazy;
use slog::Logger;

static SHARED_LOGGER: Lazy<Mutex<HashMap<String, Logger>>> =
    Lazy::new(|| Mutex::new(HashMap::new()));

pub(super) enum SharedLoggerType {
    Task,
}

pub(super) fn get_shared_logger<F>(
    logger_type: SharedLoggerType,
    logger_name: String,
    sub_logger: F,
) -> Logger
where
    F: Fn(&Logger) -> Logger,
{
    let (config, log_type) = match logger_type {
        SharedLoggerType::Task => (
            crate::config::log::get_task_default_config(),
            super::LOG_TYPE_TASK,
        ),
    };
    let mut container = SHARED_LOGGER.lock().unwrap();
    let logger = match container.entry(format!("{log_type}/{logger_name}")) {
        Entry::Occupied(entry) => entry.into_mut(),
        Entry::Vacant(entry) => {
            let logger = g3_daemon::log::create_shared_logger(
                logger_name,
                crate::config::daemon_group_name(),
                log_type,
                &config,
            );
            entry.insert(logger)
        }
    };
    sub_logger(logger)
}
