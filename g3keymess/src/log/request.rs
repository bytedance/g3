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

use slog::{slog_info, slog_o, Logger};
use uuid::Uuid;

use g3_slog_types::LtUuid;
use g3_types::metrics::NodeName;

use super::shared::SharedLoggerType;
use crate::protocol::KeylessResponse;

pub(crate) fn get_logger(server_name: &NodeName) -> Logger {
    let config = crate::config::log::get_task_default_config();
    let logger_name = format!("lr-{server_name}");
    let common_values = slog_o!(
        "daemon_name" => crate::opts::daemon_group(),
        "log_type" => super::LOG_TYPE_REQUEST,
        "pid" => std::process::id(),
        "server_name" => server_name.to_string(),
    );
    config.build_logger(logger_name, super::LOG_TYPE_REQUEST, common_values)
}

pub(crate) fn get_shared_logger(name: &str, server_name: &NodeName) -> Logger {
    let logger_name = format!("lr-{name}");
    super::shared::get_shared_logger(SharedLoggerType::Request, logger_name, |logger| {
        logger.new(slog_o!(
            "server_name" => server_name.to_string(),
        ))
    })
}

pub(crate) struct RequestErrorLogContext<'a> {
    pub(crate) task_id: &'a Uuid,
}

impl<'a> RequestErrorLogContext<'a> {
    pub(crate) fn log(&'a self, logger: &'a Logger, rsp: &KeylessResponse) {
        if let KeylessResponse::Error(r) = rsp {
            let e = r.error_code();
            slog_info!(logger, "{}", e;
                "task_id" => LtUuid(self.task_id),
                "msg_id" => r.id,
            )
        }
    }
}
