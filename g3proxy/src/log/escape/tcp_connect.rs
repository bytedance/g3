/*
 * SPDX-License-Identifier: Apache-2.0
 * Copyright 2023-2025 ByteDance and/or its affiliates.
 */

use slog::{Logger, slog_info};
use uuid::Uuid;

use g3_slog_types::{LtDateTime, LtDuration, LtIpAddr, LtUpstreamAddr, LtUuid};
use g3_types::net::UpstreamAddr;

use crate::module::tcp_connect::{TcpConnectError, TcpConnectTaskNotes};

pub(crate) struct EscapeLogForTcpConnect<'a> {
    pub(crate) upstream: &'a UpstreamAddr,
    pub(crate) tcp_notes: &'a TcpConnectTaskNotes,
    pub(crate) task_id: &'a Uuid,
}

impl EscapeLogForTcpConnect<'_> {
    pub(crate) fn log(&self, logger: &Logger, e: &TcpConnectError) {
        slog_info!(logger, "{}", e;
            "escape_type" => "TcpConnect",
            "task_id" => LtUuid(self.task_id),
            "upstream" => LtUpstreamAddr(self.upstream),
            "next_bind_ip" => self.tcp_notes.bind.ip().map(LtIpAddr),
            "next_bound_addr" => self.tcp_notes.local,
            "next_peer_addr" => self.tcp_notes.next,
            "next_expire" => self.tcp_notes.expire.as_ref().map(LtDateTime),
            "tcp_connect_tries" => self.tcp_notes.tries,
            "tcp_connect_spend" => LtDuration(self.tcp_notes.duration),
            "reason" => e.brief(),
        )
    }
}
