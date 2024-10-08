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

use async_trait::async_trait;
use tokio::time::Instant;

use g3_types::net::{Host, HttpForwardCapability, OpensslClientConfig, UpstreamAddr};

use crate::audit::AuditContext;
use crate::escape::{ArcEscaper, ArcEscaperInternalStats};
use crate::module::http_forward::{
    ArcHttpForwardTaskRemoteStats, BoxHttpForwardConnection, HttpConnectionEofPoller,
    HttpForwardContext,
};
use crate::module::tcp_connect::{TcpConnectError, TcpConnectTaskNotes};
use crate::serve::ServerTaskNotes;

pub(crate) struct ProxyHttpForwardContext {
    escaper: ArcEscaper,
    stats: ArcEscaperInternalStats,
    tcp_notes: TcpConnectTaskNotes,
    last_is_tls: bool,
    last_connection: Option<(Instant, HttpConnectionEofPoller)>,
}

impl ProxyHttpForwardContext {
    pub(crate) fn new(stats: ArcEscaperInternalStats, escaper: ArcEscaper) -> Self {
        ProxyHttpForwardContext {
            escaper,
            stats,
            tcp_notes: TcpConnectTaskNotes::empty(),
            last_is_tls: false,
            last_connection: None,
        }
    }
}

#[async_trait]
impl HttpForwardContext for ProxyHttpForwardContext {
    async fn check_in_final_escaper<'a>(
        &'a mut self,
        _task_notes: &'a ServerTaskNotes,
        _upstream: &'a UpstreamAddr,
        audit_ctx: &'a mut AuditContext,
    ) -> HttpForwardCapability {
        self.escaper._update_audit_context(audit_ctx);
        self.escaper._local_http_forward_capability()
    }

    fn prepare_connection(&mut self, ups: &UpstreamAddr, is_tls: bool) {
        if is_tls {
            self.stats.add_https_forward_request_attempted();
            if !self.last_is_tls || self.tcp_notes.upstream.ne(ups) {
                // new upstream, but not new peer
                self.tcp_notes.upstream = ups.clone();
                self.tcp_notes.reset_generated();
                // use new tls session
                let _old_connection = self.last_connection.take();
            } else {
                // old upstream and reuse tls session
            }
        } else {
            self.stats.add_http_forward_request_attempted();
            if self.last_is_tls {
                // new upstream, but not new peer
                self.tcp_notes.upstream = ups.clone();
                self.tcp_notes.reset_generated();
                // drop old tls session
                let _old_connection = self.last_connection.take();
            } else if self.tcp_notes.upstream.ne(ups) {
                // new upstream, but not new peer
                self.tcp_notes.upstream = ups.clone();
            } else {
                // old upstream
            }
        }
    }

    async fn get_alive_connection<'a>(
        &'a mut self,
        task_notes: &'a ServerTaskNotes,
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

    async fn make_new_http_connection<'a>(
        &'a mut self,
        task_notes: &'a ServerTaskNotes,
        task_stats: ArcHttpForwardTaskRemoteStats,
    ) -> Result<BoxHttpForwardConnection, TcpConnectError> {
        self.last_is_tls = false;
        self.escaper
            ._new_http_forward_connection(&mut self.tcp_notes, task_notes, task_stats)
            .await
    }

    async fn make_new_https_connection<'a>(
        &'a mut self,
        task_notes: &'a ServerTaskNotes,
        task_stats: ArcHttpForwardTaskRemoteStats,
        tls_config: &'a OpensslClientConfig,
        tls_name: &'a Host,
    ) -> Result<BoxHttpForwardConnection, TcpConnectError> {
        self.last_is_tls = true;
        self.escaper
            ._new_https_forward_connection(
                &mut self.tcp_notes,
                task_notes,
                task_stats,
                tls_config,
                tls_name,
            )
            .await
    }

    fn save_alive_connection(&mut self, c: BoxHttpForwardConnection) {
        let eof_poller = HttpConnectionEofPoller::spawn(c);
        self.last_connection = Some((Instant::now(), eof_poller));
    }

    fn fetch_tcp_notes(&self, tcp_notes: &mut TcpConnectTaskNotes) {
        // the upstream addr self.notes is the proxy_addr,
        // which is likely to be different than the one in tcp_notes
        tcp_notes.fill_generated(&self.tcp_notes);
    }
}
