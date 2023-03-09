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

use slog::{slog_o, Logger};

pub(super) fn get_logger(log_type: &'static str, auditor_name: &str) -> Logger {
    let config = crate::config::log::get_audit_default_config();
    let logger_name = format!("la-{auditor_name}");
    let common_values = slog_o!(
        "daemon_name" => crate::config::daemon_group_name(),
        "log_type" => log_type,
        "pid" => std::process::id(),
        "auditor_name" => auditor_name.to_string(),
    );
    g3_daemon::log::create_logger(&config, logger_name, log_type, common_values)
}
