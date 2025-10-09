/*
 * SPDX-License-Identifier: Apache-2.0
 * Copyright 2023-2025 ByteDance and/or its affiliates.
 */

use g3_dpi::Protocol;
use g3_slog_types::LtUuid;

use super::InspectSource;
use crate::config::server::ServerConfig;
use crate::inspect::StreamInspectContext;

pub(crate) struct StreamInspectLog<'a, SC: ServerConfig> {
    ctx: &'a StreamInspectContext<SC>,
}

impl<'a, SC: ServerConfig> StreamInspectLog<'a, SC> {
    pub(crate) fn new(ctx: &'a StreamInspectContext<SC>) -> Self {
        StreamInspectLog { ctx }
    }

    pub(crate) fn log(&self, source: InspectSource, protocol: Protocol) {
        if let Some(logger) = self.ctx.inspect_logger() {
            slog::info!(logger, "";
                "task_id" => LtUuid(self.ctx.server_task_id()),
                "depth" => self.ctx.current_inspection_depth(),
                "source" => source.as_str(),
                "protocol" => protocol.as_str(),
            );
        }
    }
}
