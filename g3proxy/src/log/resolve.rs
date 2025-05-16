/*
 * SPDX-License-Identifier: Apache-2.0
 * Copyright 2023-2025 ByteDance and/or its affiliates.
 */

use slog::{Logger, slog_o};

use g3_types::metrics::NodeName;

pub(crate) fn get_logger(resolver_type: &str, resolver_name: &NodeName) -> Option<Logger> {
    let config = crate::config::log::get_resolve_default_config();
    let logger_name = format!("lr-{resolver_name}");
    let common_values = slog_o!(
        "daemon_name" => crate::opts::daemon_group(),
        "log_type" => super::LOG_TYPE_RESOLVE,
        "pid" => std::process::id(),
        "resolver_type" => resolver_type.to_string(),
        "resolver_name" => resolver_name.to_string(),
    );
    config.build_logger(logger_name, super::LOG_TYPE_RESOLVE, common_values)
}
