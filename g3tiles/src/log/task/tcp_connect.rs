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

use std::time::Duration;

use slog::{slog_info, Logger};

use g3_slog_types::{LtDateTime, LtDuration, LtUuid};

use crate::serve::{ServerTaskError, ServerTaskNotes};

pub(crate) struct TaskLogForTcpConnect<'a> {
    pub(crate) task_notes: &'a ServerTaskNotes,
    pub(crate) total_time: Duration,
    pub(crate) client_rd_bytes: u64,
    pub(crate) client_wr_bytes: u64,
    pub(crate) remote_rd_bytes: u64,
    pub(crate) remote_wr_bytes: u64,
}

impl TaskLogForTcpConnect<'_> {
    pub(crate) fn log(&self, logger: &Logger, e: &ServerTaskError) {
        slog_info!(logger, "{}", e;
            "task_type" => "TcpConnect",
            "task_id" => LtUuid(&self.task_notes.id),
            "stage" => self.task_notes.stage.brief(),
            "start_at" => LtDateTime(&self.task_notes.start_at),
            "server_addr" => self.task_notes.server_addr,
            "client_addr" => self.task_notes.client_addr,
            "reason" => e.brief(),
            "wait_time" => LtDuration(self.task_notes.wait_time),
            "ready_time" => LtDuration(self.task_notes.ready_time),
            "total_time" => LtDuration(self.total_time),
            "c_rd_bytes" => self.client_rd_bytes,
            "c_wr_bytes" => self.client_wr_bytes,
            "r_rd_bytes" => self.remote_rd_bytes,
            "r_wr_bytes" => self.remote_wr_bytes,
        )
    }
}
