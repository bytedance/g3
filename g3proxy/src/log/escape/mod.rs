/*
 * SPDX-License-Identifier: Apache-2.0
 * Copyright 2023-2025 ByteDance and/or its affiliates.
 */

use slog::Logger;

use g3_types::metrics::NodeName;

pub(crate) mod tcp_connect;
pub(crate) mod tls_handshake;
pub(crate) mod udp_sendto;

use super::shared::SharedLoggerType;

pub(crate) fn get_logger(escaper_type: &str, escaper_name: &NodeName) -> Option<Logger> {
    let config = crate::config::log::get_escape_default_config();
    let logger_name = format!("le-{escaper_name}");
    let common_values = slog::o!(
        "daemon_name" => crate::opts::daemon_group(),
        "log_type" => super::LOG_TYPE_ESCAPE,
        "pid" => std::process::id(),
        "escaper_type" => escaper_type.to_string(),
        "escaper_name" => escaper_name.to_string(),
    );
    config.build_logger(logger_name, super::LOG_TYPE_ESCAPE, common_values)
}

pub(crate) fn get_shared_logger(
    name: &str,
    escaper_type: &str,
    escaper_name: &NodeName,
) -> Option<Logger> {
    let logger_name = format!("le-{name}");
    super::shared::get_shared_logger(SharedLoggerType::Escape, logger_name, |logger| {
        logger.new(slog::o!(
            "escaper_type" => escaper_type.to_string(),
            "escaper_name" => escaper_name.to_string(),
        ))
    })
}
