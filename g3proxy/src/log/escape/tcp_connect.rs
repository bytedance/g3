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

use slog::{slog_info, Logger};
use uuid::Uuid;

use g3_slog_types::{LtDateTime, LtDuration, LtIpAddr, LtUpstreamAddr, LtUuid};

use crate::module::tcp_connect::{TcpConnectError, TcpConnectTaskNotes};

pub(crate) struct EscapeLogForTcpConnect<'a> {
    pub(crate) tcp_notes: &'a TcpConnectTaskNotes,
    pub(crate) task_id: &'a Uuid,
}

impl EscapeLogForTcpConnect<'_> {
    pub(crate) fn log(&self, logger: &Logger, e: &TcpConnectError) {
        slog_info!(logger, "{}", e;
            "escape_type" => "TcpConnect",
            "task_id" => LtUuid(self.task_id),
            "upstream" => LtUpstreamAddr(&self.tcp_notes.upstream),
            "next_bind_ip" => self.tcp_notes.bind.map(LtIpAddr),
            "next_bound_addr" => self.tcp_notes.local,
            "next_peer_addr" => self.tcp_notes.next,
            "next_expire" => self.tcp_notes.expire.as_ref().map(LtDateTime),
            "tcp_connect_tries" => self.tcp_notes.tries,
            "tcp_connect_spend" => LtDuration(self.tcp_notes.duration),
            "reason" => e.brief(),
        )
    }
}
