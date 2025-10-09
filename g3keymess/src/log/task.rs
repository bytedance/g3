/*
 * SPDX-License-Identifier: Apache-2.0
 * Copyright 2023-2025 ByteDance and/or its affiliates.
 */

use slog::Logger;

use g3_types::metrics::NodeName;

use super::shared::SharedLoggerType;

pub(crate) fn get_logger(server_name: &NodeName) -> Option<Logger> {
    let config = crate::config::log::get_task_default_config();
    let logger_name = format!("lt-{server_name}");
    let common_values = slog::o!(
        "daemon_name" => crate::opts::daemon_group(),
        "log_type" => super::LOG_TYPE_TASK,
        "pid" => std::process::id(),
        "server_name" => server_name.to_string(),
    );
    config.build_logger(logger_name, super::LOG_TYPE_TASK, common_values)
}

pub(crate) fn get_shared_logger(name: &str, server_name: &NodeName) -> Option<Logger> {
    let logger_name = format!("lt-{name}");
    super::shared::get_shared_logger(SharedLoggerType::Task, logger_name, |logger| {
        logger.new(slog::o!(
            "server_name" => server_name.to_string(),
        ))
    })
}
