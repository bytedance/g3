/*
 * SPDX-License-Identifier: Apache-2.0
 * Copyright 2023-2025 ByteDance and/or its affiliates.
 */

use std::time::Duration;

use async_trait::async_trait;
use tokio::time::Instant;

use g3_types::net::{HttpForwardCapability, UpstreamAddr};

use super::{
    ArcHttpForwardTaskRemoteStats, BoxHttpForwardConnection, HttpConnectionEofPoller,
    HttpForwardContext,
};
use crate::audit::AuditContext;
use crate::escape::{ArcEscaper, ArcEscaperInternalStats};
use crate::module::tcp_connect::{
    TcpConnectError, TcpConnectTaskConf, TcpConnectTaskNotes, TlsConnectTaskConf,
};
use crate::serve::ServerTaskNotes;

pub(crate) struct DirectHttpForwardContext {
    escaper: ArcEscaper,
    stats: ArcEscaperInternalStats,
    tcp_notes: TcpConnectTaskNotes,
    last_upstream: UpstreamAddr,
    last_is_tls: bool,
    last_connection: Option<(Instant, HttpConnectionEofPoller)>,
}

impl DirectHttpForwardContext {
    pub(crate) fn new(stats: ArcEscaperInternalStats, escaper: ArcEscaper) -> Self {
        DirectHttpForwardContext {
            escaper,
            stats,
            tcp_notes: TcpConnectTaskNotes::default(),
            last_upstream: UpstreamAddr::empty(),
            last_is_tls: false,
            last_connection: None,
        }
    }
}

#[async_trait]
impl HttpForwardContext for DirectHttpForwardContext {
    async fn check_in_final_escaper(
        &mut self,
        _task_notes: &ServerTaskNotes,
        _upstream: &UpstreamAddr,
        audit_ctx: &mut AuditContext,
    ) -> HttpForwardCapability {
        self.escaper._update_audit_context(audit_ctx);
        self.escaper._local_http_forward_capability()
    }

    fn prepare_connection(&mut self, ups: &UpstreamAddr, is_tls: bool) {
        if is_tls {
            self.stats.add_https_forward_request_attempted();
        } else {
            self.stats.add_http_forward_request_attempted();
        }

        if self.last_upstream.ne(ups) || self.last_is_tls != is_tls {
            // new upstream
            self.last_upstream = ups.clone();
            self.tcp_notes.reset();
            // always use different connection for different upstream
            let _old_connection = self.last_connection.take();
        } else {
            // old upstream
        }
    }

    async fn get_alive_connection(
        &mut self,
        task_notes: &ServerTaskNotes,
        task_stats: ArcHttpForwardTaskRemoteStats,
        idle_expire: Duration,
    ) -> Option<BoxHttpForwardConnection> {
        let all_user_stats = task_notes
            .user_ctx()
            .map(|ctx| {
                self.escaper
                    .get_escape_stats()
                    .map(|s| ctx.fetch_upstream_traffic_stats(s.name(), s.share_extra_tags()))
                    .unwrap_or_default()
            })
            .unwrap_or_default();

        let (instant, eof_poller) = self.last_connection.take()?;
        if instant.elapsed() < idle_expire {
            let mut connection = eof_poller.recv_conn().await?;
            connection
                .0
                .update_stats(&task_stats, all_user_stats.clone());
            connection.1.update_stats(&task_stats, all_user_stats);
            Some(connection)
        } else {
            None
        }
    }

    async fn make_new_http_connection(
        &mut self,
        task_conf: &TcpConnectTaskConf<'_>,
        task_notes: &ServerTaskNotes,
        task_stats: ArcHttpForwardTaskRemoteStats,
    ) -> Result<BoxHttpForwardConnection, TcpConnectError> {
        self.last_is_tls = false;
        self.escaper
            ._new_http_forward_connection(task_conf, &mut self.tcp_notes, task_notes, task_stats)
            .await
    }

    async fn make_new_https_connection(
        &mut self,
        task_conf: &TlsConnectTaskConf<'_>,
        task_notes: &ServerTaskNotes,
        task_stats: ArcHttpForwardTaskRemoteStats,
    ) -> Result<BoxHttpForwardConnection, TcpConnectError> {
        self.last_is_tls = true;
        self.escaper
            ._new_https_forward_connection(task_conf, &mut self.tcp_notes, task_notes, task_stats)
            .await
    }

    fn save_alive_connection(&mut self, c: BoxHttpForwardConnection) {
        let eof_poller = HttpConnectionEofPoller::spawn(c);
        self.last_connection = Some((Instant::now(), eof_poller));
    }

    fn fetch_tcp_notes(&self, tcp_notes: &mut TcpConnectTaskNotes) {
        tcp_notes.clone_from(&self.tcp_notes);
    }
}
