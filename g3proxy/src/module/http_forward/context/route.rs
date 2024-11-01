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

use std::sync::Arc;
use std::time::Duration;

use async_trait::async_trait;
use tokio::time::Instant;

use g3_types::net::{HttpForwardCapability, UpstreamAddr};

use super::{
    ArcHttpForwardTaskRemoteStats, BoxHttpForwardConnection, HttpConnectionEofPoller,
    HttpForwardContext,
};
use crate::audit::AuditContext;
use crate::escape::ArcEscaper;
use crate::module::tcp_connect::{
    TcpConnectError, TcpConnectTaskConf, TcpConnectTaskNotes, TlsConnectTaskConf,
};
use crate::serve::ServerTaskNotes;

pub(crate) struct RouteHttpForwardContext {
    escaper: ArcEscaper,
    final_escaper: ArcEscaper,
    tcp_notes: TcpConnectTaskNotes,
    audit_ctx: AuditContext,
    last_upstream: UpstreamAddr,
    last_is_tls: bool,
    last_connection: Option<(Instant, HttpConnectionEofPoller)>,
}

impl RouteHttpForwardContext {
    pub(crate) fn new(escaper: ArcEscaper) -> Self {
        let fake_final_escaper = Arc::clone(&escaper);
        RouteHttpForwardContext {
            escaper,
            final_escaper: fake_final_escaper,
            tcp_notes: TcpConnectTaskNotes::default(),
            audit_ctx: AuditContext::default(),
            last_upstream: UpstreamAddr::empty(),
            last_is_tls: false,
            last_connection: None,
        }
    }
}

#[async_trait]
impl HttpForwardContext for RouteHttpForwardContext {
    async fn check_in_final_escaper(
        &mut self,
        task_notes: &ServerTaskNotes,
        upstream: &UpstreamAddr,
        audit_ctx: &mut AuditContext,
    ) -> HttpForwardCapability {
        if self.last_upstream.ne(upstream) {
            self.audit_ctx = audit_ctx.clone();
            let mut next_escaper = Arc::clone(&self.escaper);
            next_escaper._update_audit_context(&mut self.audit_ctx);
            while let Some(escaper) = next_escaper
                ._check_out_next_escaper(task_notes, upstream)
                .await
            {
                next_escaper = escaper;
                next_escaper._update_audit_context(&mut self.audit_ctx);
            }
            if !Arc::ptr_eq(&self.final_escaper, &next_escaper) {
                self.final_escaper = next_escaper;
                // drop the old connection on old escaper
                let _old_connection = self.last_connection.take();
            }
        }

        *audit_ctx = self.audit_ctx.clone();
        self.final_escaper._local_http_forward_capability()
    }

    fn prepare_connection(&mut self, ups: &UpstreamAddr, is_tls: bool) {
        if let Some(final_stats) = self.final_escaper.get_escape_stats() {
            if is_tls {
                final_stats.add_https_forward_request_attempted();
            } else {
                final_stats.add_http_forward_request_attempted();
            }
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
                self.final_escaper
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
        self.final_escaper
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
        self.final_escaper
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
