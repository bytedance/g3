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

use g3_types::metrics::NodeName;

pub(crate) mod ftp_over_http;
pub(crate) mod http_forward;
pub(crate) mod tcp_connect;
pub(crate) mod udp_associate;
pub(crate) mod udp_connect;

use super::shared::SharedLoggerType;

pub(crate) fn get_logger(server_type: &str, server_name: &NodeName) -> Logger {
    let config = crate::config::log::get_task_default_config();
    let logger_name = format!("lt-{server_name}");
    let common_values = slog_o!(
        "daemon_name" => crate::opts::daemon_group(),
        "log_type" => super::LOG_TYPE_TASK,
        "pid" => std::process::id(),
        "server_type" => server_type.to_string(),
        "server_name" => server_name.to_string(),
    );
    config.build_logger(logger_name, super::LOG_TYPE_TASK, common_values)
}

pub(crate) fn get_shared_logger(name: &str, server_type: &str, server_name: &NodeName) -> Logger {
    let logger_name = format!("lt-{name}");
    super::shared::get_shared_logger(SharedLoggerType::Task, logger_name, |logger| {
        logger.new(slog_o!(
            "server_type" => server_type.to_string(),
            "server_name" => server_name.to_string(),
        ))
    })
}

enum TaskEvent {
    Created,
    Connected,
    Periodic,
    Finished,
}

impl TaskEvent {
    fn as_str(&self) -> &'static str {
        match self {
            TaskEvent::Created => "created",
            TaskEvent::Connected => "connected",
            TaskEvent::Periodic => "periodic",
            TaskEvent::Finished => "finished",
        }
    }
}
