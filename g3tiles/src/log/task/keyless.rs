/*
 * SPDX-License-Identifier: Apache-2.0
 * Copyright 2024-2025 ByteDance and/or its affiliates.
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
