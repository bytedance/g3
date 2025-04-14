/*
 * Copyright 2024 ByteDance and/or its affiliates.
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

use slog::{Logger, slog_info};

use g3_slog_types::{LtDateTime, LtDuration, LtUuid};

use crate::module::keyless::KeylessRelaySnapshot;
use crate::serve::{ServerTaskError, ServerTaskNotes};

pub(crate) struct TaskLogForKeyless<'a> {
    pub(crate) logger: &'a Logger,
    pub(crate) task_notes: &'a ServerTaskNotes,
    pub(crate) task_stats: KeylessRelaySnapshot,
}

impl TaskLogForKeyless<'_> {
    pub(crate) fn log(&self, e: ServerTaskError) {
        slog_info!(self.logger, "{}", e;
            "task_type" => "Keyless",
            "task_id" => LtUuid(&self.task_notes.id),
            "stage" => self.task_notes.stage.brief(),
            "start_at" => LtDateTime(&self.task_notes.start_at),
            "server_addr" => self.task_notes.server_addr(),
            "client_addr" => self.task_notes.client_addr(),
            "reason" => e.brief(),
            "wait_time" => LtDuration(self.task_notes.wait_time),
            "ready_time" => LtDuration(self.task_notes.ready_time),
            "total_time" => LtDuration(self.task_notes.time_elapsed()),
            "req_total" => self.task_stats.req_total,
            "req_pass" => self.task_stats.req_pass,
            "req_fail" => self.task_stats.req_fail,
            "rsp_drop" => self.task_stats.rsp_drop,
            "rsp_pass" => self.task_stats.rsp_pass,
            "rsp_fail" => self.task_stats.rsp_fail,
        )
    }
}
