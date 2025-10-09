/*
 * SPDX-License-Identifier: Apache-2.0
 * Copyright 2023-2025 ByteDance and/or its affiliates.
 */

use slog::{KV, Logger, Record, Serializer, Value};
use uuid::Uuid;

use g3_slog_types::{LtDateTime, LtDuration, LtUuid};
use g3_types::metrics::NodeName;

use super::shared::SharedLoggerType;
use crate::protocol::KeylessResponse;
use crate::serve::RequestProcessContext;

pub(crate) fn get_logger(server_name: &NodeName) -> Option<Logger> {
    let config = crate::config::log::get_task_default_config();
    let logger_name = format!("lr-{server_name}");
    let common_values = slog::o!(
        "daemon_name" => crate::opts::daemon_group(),
        "log_type" => super::LOG_TYPE_REQUEST,
        "pid" => std::process::id(),
        "server_name" => server_name.to_string(),
    );
    config.build_logger(logger_name, super::LOG_TYPE_REQUEST, common_values)
}

pub(crate) fn get_shared_logger(name: &str, server_name: &NodeName) -> Option<Logger> {
    let logger_name = format!("lr-{name}");
    super::shared::get_shared_logger(SharedLoggerType::Request, logger_name, |logger| {
        logger.new(slog::o!(
            "server_name" => server_name.to_string(),
        ))
    })
}

struct RequestLogKv<'a> {
    task_id: &'a Uuid,
    ctx: &'a RequestProcessContext,
}

impl KV for RequestLogKv<'_> {
    fn serialize(&self, record: &Record, serializer: &mut dyn Serializer) -> slog::Result {
        LtUuid(self.task_id).serialize(record, "task_id".into(), serializer)?;
        serializer.emit_u32("msg_id".into(), self.ctx.msg_id)?;
        LtDateTime(&self.ctx.create_datetime).serialize(record, "create_at".into(), serializer)?;
        LtDuration(self.ctx.duration()).serialize(record, "process_time".into(), serializer)?;
        Ok(())
    }
}

pub(crate) struct RequestErrorLogContext<'a> {
    pub(crate) task_id: &'a Uuid,
}

impl<'a> RequestErrorLogContext<'a> {
    pub(crate) fn log(
        &'a self,
        logger: &'a Logger,
        ctx: &RequestProcessContext,
        rsp: &KeylessResponse,
    ) {
        let log_kv = RequestLogKv {
            task_id: self.task_id,
            ctx,
        };
        if let KeylessResponse::Error(r) = rsp {
            let e = r.error_code();
            slog::info!(logger, "{}", e; log_kv);
        }
    }
}
