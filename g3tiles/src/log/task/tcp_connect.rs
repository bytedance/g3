/*
 * SPDX-License-Identifier: Apache-2.0
 * Copyright 2023-2025 ByteDance and/or its affiliates.
 */

use slog::{Logger, slog_info};

use g3_slog_types::{LtDateTime, LtDuration, LtUuid};

use super::TaskEvent;
use crate::serve::{ServerTaskError, ServerTaskNotes};

pub(crate) struct TaskLogForTcpConnect<'a> {
    pub(crate) logger: &'a Logger,
    pub(crate) task_notes: &'a ServerTaskNotes,
    pub(crate) client_rd_bytes: u64,
    pub(crate) client_wr_bytes: u64,
    pub(crate) remote_rd_bytes: u64,
    pub(crate) remote_wr_bytes: u64,
}

impl TaskLogForTcpConnect<'_> {
    pub(crate) fn log_created(&self) {
        slog_info!(self.logger, "";
            "task_type" => "TcpConnect",
            "task_id" => LtUuid(&self.task_notes.id),
            "task_event" => TaskEvent::Created.as_str(),
            "stage" => self.task_notes.stage.brief(),
            "start_at" => LtDateTime(&self.task_notes.start_at),
            "server_addr" => self.task_notes.server_addr(),
            "client_addr" => self.task_notes.client_addr(),
            "wait_time" => LtDuration(self.task_notes.wait_time),
        )
    }

    pub(crate) fn log_connected(&self) {
        slog_info!(self.logger, "";
            "task_type" => "TcpConnect",
            "task_id" => LtUuid(&self.task_notes.id),
            "task_event" => TaskEvent::Connected.as_str(),
            "stage" => self.task_notes.stage.brief(),
            "start_at" => LtDateTime(&self.task_notes.start_at),
            "server_addr" => self.task_notes.server_addr(),
            "client_addr" => self.task_notes.client_addr(),
            "wait_time" => LtDuration(self.task_notes.wait_time),
            "ready_time" => LtDuration(self.task_notes.ready_time),
        )
    }

    pub(crate) fn log_periodic(&self) {
        slog_info!(self.logger, "";
            "task_type" => "TcpConnect",
            "task_id" => LtUuid(&self.task_notes.id),
            "task_event" => TaskEvent::Periodic.as_str(),
            "stage" => self.task_notes.stage.brief(),
            "start_at" => LtDateTime(&self.task_notes.start_at),
            "server_addr" => self.task_notes.server_addr(),
            "client_addr" => self.task_notes.client_addr(),
            "wait_time" => LtDuration(self.task_notes.wait_time),
            "ready_time" => LtDuration(self.task_notes.ready_time),
            "total_time" => LtDuration(self.task_notes.time_elapsed()),
            "c_rd_bytes" => self.client_rd_bytes,
            "c_wr_bytes" => self.client_wr_bytes,
            "r_rd_bytes" => self.remote_rd_bytes,
            "r_wr_bytes" => self.remote_wr_bytes,
        )
    }

    fn log_partial_shutdown(&self, task_event: TaskEvent) {
        slog_info!(self.logger, "";
            "task_type" => "TcpConnect",
            "task_id" => LtUuid(&self.task_notes.id),
            "task_event" => task_event.as_str(),
            "stage" => self.task_notes.stage.brief(),
            "start_at" => LtDateTime(&self.task_notes.start_at),
            "server_addr" => self.task_notes.server_addr(),
            "client_addr" => self.task_notes.client_addr(),
            "wait_time" => LtDuration(self.task_notes.wait_time),
            "ready_time" => LtDuration(self.task_notes.ready_time),
            "total_time" => LtDuration(self.task_notes.time_elapsed()),
            "c_rd_bytes" => self.client_rd_bytes,
            "c_wr_bytes" => self.client_wr_bytes,
            "r_rd_bytes" => self.remote_rd_bytes,
            "r_wr_bytes" => self.remote_wr_bytes,
        )
    }

    pub(crate) fn log_client_shutdown(&self) {
        self.log_partial_shutdown(TaskEvent::ClientShutdown);
    }

    pub(crate) fn log_upstream_shutdown(&self) {
        self.log_partial_shutdown(TaskEvent::UpstreamShutdown);
    }

    pub(crate) fn log(&self, e: ServerTaskError) {
        slog_info!(self.logger, "{}", e;
            "task_type" => "TcpConnect",
            "task_id" => LtUuid(&self.task_notes.id),
            "task_event" => TaskEvent::Finished.as_str(),
            "stage" => self.task_notes.stage.brief(),
            "start_at" => LtDateTime(&self.task_notes.start_at),
            "server_addr" => self.task_notes.server_addr(),
            "client_addr" => self.task_notes.client_addr(),
            "reason" => e.brief(),
            "wait_time" => LtDuration(self.task_notes.wait_time),
            "ready_time" => LtDuration(self.task_notes.ready_time),
            "total_time" => LtDuration(self.task_notes.time_elapsed()),
            "c_rd_bytes" => self.client_rd_bytes,
            "c_wr_bytes" => self.client_wr_bytes,
            "r_rd_bytes" => self.remote_rd_bytes,
            "r_wr_bytes" => self.remote_wr_bytes,
        )
    }
}
