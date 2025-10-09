/*
 * SPDX-License-Identifier: Apache-2.0
 * Copyright 2023-2025 ByteDance and/or its affiliates.
 */

use slog::Logger;
use uuid::Uuid;

use g3_slog_types::{LtDateTime, LtHost, LtIpAddr, LtUpstreamAddr, LtUuid};
use g3_types::net::{Host, UpstreamAddr};

use crate::module::tcp_connect::TcpConnectTaskNotes;

pub(crate) struct EscapeLogForTlsHandshake<'a> {
    pub(crate) upstream: &'a UpstreamAddr,
    pub(crate) tcp_notes: &'a TcpConnectTaskNotes,
    pub(crate) task_id: &'a Uuid,
    pub(crate) tls_name: &'a Host,
    pub(crate) tls_peer: &'a UpstreamAddr,
    pub(crate) tls_application: TlsApplication,
}

pub(crate) enum TlsApplication {
    HttpForward,
    HttpProxy,
    TcpStream,
}

impl TlsApplication {
    const fn as_str(&self) -> &'static str {
        match self {
            Self::HttpForward => "HttpForward",
            Self::HttpProxy => "HttpProxy",
            Self::TcpStream => "TcpStream",
        }
    }
}

impl EscapeLogForTlsHandshake<'_> {
    pub(crate) fn log(&self, logger: &Logger, e: &anyhow::Error) {
        slog::info!(logger, "{:?}", e;
            "escape_type" => "TlsHandshake",
            "task_id" => LtUuid(self.task_id),
            "upstream" => LtUpstreamAddr(self.upstream),
            "next_bind_ip" => self.tcp_notes.bind.ip().map(LtIpAddr),
            "next_bound_addr" => self.tcp_notes.local,
            "next_peer_addr" => self.tcp_notes.next,
            "next_expire" => self.tcp_notes.expire.as_ref().map(LtDateTime),
            "tls_name" => LtHost(self.tls_name),
            "tls_peer" => LtUpstreamAddr(self.tls_peer),
            "tls_application" => self.tls_application.as_str(),
        )
    }
}
