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

use g3_daemon::log::types::{LtDateTime, LtDuration, LtIpAddr, LtUpstreamAddr, LtUuid};

use crate::log::types::{LtHttpMethod, LtHttpUri};
use crate::module::ftp_over_http::FtpOverHttpTaskNotes;
use crate::serve::{ServerTaskError, ServerTaskNotes};

pub(crate) struct TaskLogForFtpOverHttp<'a> {
    pub(crate) task_notes: &'a ServerTaskNotes,
    pub(crate) ftp_notes: &'a FtpOverHttpTaskNotes,
    pub(crate) http_user_agent: Option<&'a str>,
    pub(crate) total_time: Duration,
    pub(crate) client_rd_bytes: u64,
    pub(crate) client_wr_bytes: u64,
    pub(crate) ftp_c_rd_bytes: u64,
    pub(crate) ftp_c_wr_bytes: u64,
    pub(crate) ftp_d_rd_bytes: u64,
    pub(crate) ftp_d_wr_bytes: u64,
}

impl TaskLogForFtpOverHttp<'_> {
    pub(crate) fn log(&self, logger: &Logger, e: &ServerTaskError) {
        let username = if let Some(user_ctx) = self.task_notes.user_ctx() {
            if user_ctx.skip_log() {
                return;
            }
            user_ctx.user().name()
        } else {
            ""
        };

        slog_info!(logger, "{}", e;
            "task_type" => "FtpOverHttp",
            "task_id" => LtUuid(&self.task_notes.id),
            "stage" => self.task_notes.stage.brief(),
            "start_at" => LtDateTime(&self.task_notes.start_at),
            "user" => username,
            "server_addr" => self.task_notes.server_addr,
            "client_addr" => self.task_notes.client_addr,
            "upstream" => LtUpstreamAddr(self.ftp_notes.upstream()),
            "escaper" => &self.ftp_notes.control_tcp_notes.escaper,
            "next_bind_ip" => self.ftp_notes.control_tcp_notes.bind.map(LtIpAddr),
            "next_expire" => self.ftp_notes.control_tcp_notes.expire.as_ref().map(LtDateTime),
            "ftp_c_bound_addr" => self.ftp_notes.control_tcp_notes.local,
            "ftp_c_peer_addr" => self.ftp_notes.control_tcp_notes.next,
            "ftp_c_connect_tries" => self.ftp_notes.control_tcp_notes.tries,
            "ftp_c_connect_spend" => LtDuration(self.ftp_notes.control_tcp_notes.duration),
            "ftp_d_bound_addr" => self.ftp_notes.transfer_tcp_notes.local,
            "ftp_d_peer_addr" => self.ftp_notes.transfer_tcp_notes.next,
            "ftp_d_connect_tries" => self.ftp_notes.transfer_tcp_notes.tries,
            "ftp_d_connect_spend" => LtDuration(self.ftp_notes.transfer_tcp_notes.duration),
            "reason" => e.brief(),
            "method" => LtHttpMethod(&self.ftp_notes.method),
            "uri" => LtHttpUri::new(&self.ftp_notes.uri, self.ftp_notes.uri_log_max_chars),
            "user_agent" => self.http_user_agent,
            "rsp_status" => self.ftp_notes.rsp_status,
            "wait_time" => LtDuration(self.task_notes.wait_time),
            "ready_time" => LtDuration(self.task_notes.ready_time),
            "total_time" => LtDuration(self.total_time),
            "c_rd_bytes" => self.client_rd_bytes,
            "c_wr_bytes" => self.client_wr_bytes,
            "ftp_c_rd_bytes" => self.ftp_c_rd_bytes,
            "ftp_c_wr_bytes" => self.ftp_c_wr_bytes,
            "ftp_d_rd_bytes" => self.ftp_d_rd_bytes,
            "ftp_d_wr_bytes" => self.ftp_d_wr_bytes,
        )
    }
}
