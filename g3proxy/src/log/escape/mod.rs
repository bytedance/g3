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

use slog::{Logger, slog_o};

use g3_types::metrics::NodeName;

pub(crate) mod tcp_connect;
pub(crate) mod tls_handshake;
pub(crate) mod udp_sendto;

use super::shared::SharedLoggerType;

pub(crate) fn get_logger(escaper_type: &str, escaper_name: &NodeName) -> Option<Logger> {
    let config = crate::config::log::get_escape_default_config();
    let logger_name = format!("le-{escaper_name}");
    let common_values = slog_o!(
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
        logger.new(slog_o!(
            "escaper_type" => escaper_type.to_string(),
            "escaper_name" => escaper_name.to_string(),
        ))
    })
}
