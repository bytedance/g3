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

use slog::slog_info;

use g3_daemon::log::types::LtUuid;
use g3_dpi::Protocol;

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
        slog_info!(self.ctx.inspect_logger(), "";
            "task_id" => LtUuid(self.ctx.server_task_id()),
            "depth" => self.ctx.current_inspection_depth(),
            "source" => source.as_str(),
            "protocol" => protocol.as_str(),
        )
    }
}
