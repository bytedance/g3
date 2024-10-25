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

use std::pin::pin;
use std::sync::Arc;
use std::time::Duration;

use anyhow::anyhow;
use async_trait::async_trait;
use tokio::time::Instant;

use g3_types::net::{HttpForwardCapability, UpstreamAddr};

use super::{
    ArcHttpForwardTaskRemoteStats, BoxHttpForwardConnection, HttpConnectionEofPoller,
    HttpForwardContext,
};
use crate::audit::AuditContext;
use crate::escape::{ArcEscaper, RouteEscaperStats};
use crate::module::tcp_connect::{
    TcpConnectError, TcpConnectTaskConf, TcpConnectTaskNotes, TlsConnectTaskConf,
};
use crate::serve::ServerTaskNotes;

struct HttpConnectFailoverContext {
    tcp_notes: TcpConnectTaskNotes,
    escaper: ArcEscaper,
    connect_result: Result<BoxHttpForwardConnection, TcpConnectError>,
}

impl HttpConnectFailoverContext {
    fn new(escaper: ArcEscaper) -> Self {
        HttpConnectFailoverContext {
            tcp_notes: TcpConnectTaskNotes::default(),
            escaper,
            connect_result: Err(TcpConnectError::EscaperNotUsable(anyhow!(
                "no http connection tried yet"
            ))),
        }
    }

    async fn run_http(
        mut self,
        task_conf: &TcpConnectTaskConf<'_>,
        task_notes: &ServerTaskNotes,
        task_stats: ArcHttpForwardTaskRemoteStats,
    ) -> Result<Self, Self> {
        match self
            .escaper
            ._new_http_forward_connection(task_conf, &mut self.tcp_notes, task_notes, task_stats)
            .await
        {
            Ok(c) => {
                self.connect_result = Ok(c);
                Ok(self)
            }
            Err(e) => {
                self.connect_result = Err(e);
                Err(self)
            }
        }
    }

    async fn run_https(
        mut self,
        task_conf: &TlsConnectTaskConf<'_>,
        task_notes: &ServerTaskNotes,
        task_stats: ArcHttpForwardTaskRemoteStats,
    ) -> Result<Self, Self> {
        match self
            .escaper
            ._new_https_forward_connection(task_conf, &mut self.tcp_notes, task_notes, task_stats)
            .await
        {
            Ok(c) => {
                self.connect_result = Ok(c);
                Ok(self)
            }
            Err(e) => {
                self.connect_result = Err(e);
                Err(self)
            }
        }
    }
}

pub(crate) struct FailoverHttpForwardContext {
    route_stats: Arc<RouteEscaperStats>,
    fallback_delay: Duration,
    primary_escaper: ArcEscaper,
    standby_escaper: ArcEscaper,
    primary_final_escaper: ArcEscaper,
    standby_final_escaper: ArcEscaper,
    use_primary: bool,
    used_escaper: ArcEscaper,
    tcp_notes: TcpConnectTaskNotes,
    audit_ctx: AuditContext,
    last_upstream: UpstreamAddr,
    last_is_tls: bool,
    last_connection: Option<(Instant, HttpConnectionEofPoller)>,
}

impl FailoverHttpForwardContext {
    pub(crate) fn new(
        primary_escaper: &ArcEscaper,
        standby_escaper: &ArcEscaper,
        fallback_delay: Duration,
        route_stats: Arc<RouteEscaperStats>,
    ) -> Self {
        FailoverHttpForwardContext {
            route_stats,
            fallback_delay,
            primary_escaper: Arc::clone(primary_escaper),
            standby_escaper: Arc::clone(standby_escaper),
            primary_final_escaper: Arc::clone(primary_escaper),
            standby_final_escaper: Arc::clone(standby_escaper),
            use_primary: true,
            used_escaper: Arc::clone(primary_escaper),
            tcp_notes: TcpConnectTaskNotes::default(),
            audit_ctx: AuditContext::default(),
            last_upstream: UpstreamAddr::empty(),
            last_is_tls: false,
            last_connection: None,
        }
    }
}

#[async_trait]
impl HttpForwardContext for FailoverHttpForwardContext {
    async fn check_in_final_escaper<'a>(
        &'a mut self,
        task_notes: &'a ServerTaskNotes,
        upstream: &'a UpstreamAddr,
        audit_ctx: &'a mut AuditContext,
    ) -> HttpForwardCapability {
        if self.last_upstream.ne(upstream) {
            self.audit_ctx = audit_ctx.clone();
            // only use audit ctx of the primary escaper

            let mut primary_next_escaper = Arc::clone(&self.primary_escaper);
            primary_next_escaper._update_audit_context(&mut self.audit_ctx);
            while let Some(escaper) = primary_next_escaper
                ._check_out_next_escaper(task_notes, upstream)
                .await
            {
                primary_next_escaper = escaper;
                primary_next_escaper._update_audit_context(&mut self.audit_ctx);
            }

            let mut standby_next_escaper = Arc::clone(&self.standby_escaper);
            while let Some(escaper) = standby_next_escaper
                ._check_out_next_escaper(task_notes, upstream)
                .await
            {
                standby_next_escaper = escaper;
            }

            if self.use_primary {
                if !Arc::ptr_eq(&self.primary_final_escaper, &primary_next_escaper) {
                    self.primary_final_escaper = primary_next_escaper;
                    // drop the old connection on old escaper
                    let _old_connection = self.last_connection.take();
                }
            } else if !Arc::ptr_eq(&self.standby_final_escaper, &standby_next_escaper) {
                self.standby_final_escaper = standby_next_escaper;
                // drop the old connection on old escaper
                let _old_connection = self.last_connection.take();
            }
        }

        *audit_ctx = self.audit_ctx.clone();
        self.primary_final_escaper._local_http_forward_capability()
            & self.standby_final_escaper._local_http_forward_capability()
    }

    fn prepare_connection(&mut self, ups: &UpstreamAddr, is_tls: bool) {
        if let Some(final_stats) = self.used_escaper.get_escape_stats() {
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

    async fn get_alive_connection<'a>(
        &'a mut self,
        task_notes: &'a ServerTaskNotes,
        task_stats: ArcHttpForwardTaskRemoteStats,
        idle_expire: Duration,
    ) -> Option<BoxHttpForwardConnection> {
        let all_user_stats = task_notes
            .user_ctx()
            .map(|ctx| {
                self.used_escaper
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
        task_conf: &TcpConnectTaskConf<'_>,
        task_notes: &'a ServerTaskNotes,
        task_stats: ArcHttpForwardTaskRemoteStats,
    ) -> Result<BoxHttpForwardConnection, TcpConnectError> {
        self.last_is_tls = false;

        let primary_context = HttpConnectFailoverContext::new(self.primary_final_escaper.clone());
        let mut primary_task =
            pin!(primary_context.run_http(task_conf, task_notes, task_stats.clone()));

        match tokio::time::timeout(self.fallback_delay, &mut primary_task).await {
            Ok(Ok(ctx)) => {
                if !Arc::ptr_eq(&self.used_escaper, &ctx.escaper) {
                    if let Some(escaper_stats) = ctx.escaper.get_escape_stats() {
                        escaper_stats.add_http_forward_request_attempted();
                    }
                    self.used_escaper = ctx.escaper;
                }
                self.use_primary = true;
                self.tcp_notes.clone_from(&ctx.tcp_notes);
                self.route_stats.add_request_passed();
                return ctx.connect_result;
            }
            Ok(Err(_)) => {
                if !Arc::ptr_eq(&self.used_escaper, &self.standby_final_escaper) {
                    if let Some(escaper_stats) = self.standby_final_escaper.get_escape_stats() {
                        escaper_stats.add_http_forward_request_attempted();
                    }
                    self.used_escaper = self.standby_final_escaper.clone();
                }
                self.use_primary = false;
                return match self
                    .used_escaper
                    ._new_http_forward_connection(
                        task_conf,
                        &mut self.tcp_notes,
                        task_notes,
                        task_stats,
                    )
                    .await
                {
                    Ok(c) => {
                        self.route_stats.add_request_passed();
                        Ok(c)
                    }
                    Err(e) => {
                        self.route_stats.add_request_failed();
                        Err(e)
                    }
                };
            }
            Err(_) => {}
        }

        let standby_context = HttpConnectFailoverContext::new(self.standby_final_escaper.clone());
        let standby_task = pin!(standby_context.run_http(task_conf, task_notes, task_stats));

        let ctx = match futures_util::future::select_ok([primary_task, standby_task]).await {
            Ok((ctx, _left)) => {
                self.route_stats.add_request_passed();
                ctx
            }
            Err(ctx) => {
                self.route_stats.add_request_failed();
                ctx
            }
        };
        if !Arc::ptr_eq(&self.used_escaper, &ctx.escaper) {
            if let Some(escaper_stats) = ctx.escaper.get_escape_stats() {
                escaper_stats.add_http_forward_request_attempted();
            }
            self.used_escaper = ctx.escaper;
        }
        self.use_primary = Arc::ptr_eq(&self.used_escaper, &self.primary_final_escaper);
        self.tcp_notes.clone_from(&ctx.tcp_notes);
        ctx.connect_result
    }

    async fn make_new_https_connection<'a>(
        &'a mut self,
        task_conf: &TlsConnectTaskConf<'_>,
        task_notes: &'a ServerTaskNotes,
        task_stats: ArcHttpForwardTaskRemoteStats,
    ) -> Result<BoxHttpForwardConnection, TcpConnectError> {
        self.last_is_tls = true;

        let primary_context = HttpConnectFailoverContext::new(self.primary_final_escaper.clone());
        let mut primary_task =
            pin!(primary_context.run_https(task_conf, task_notes, task_stats.clone()));

        match tokio::time::timeout(self.fallback_delay, &mut primary_task).await {
            Ok(Ok(ctx)) => {
                if !Arc::ptr_eq(&self.used_escaper, &ctx.escaper) {
                    if let Some(escaper_stats) = ctx.escaper.get_escape_stats() {
                        escaper_stats.add_https_forward_request_attempted();
                    }
                    self.used_escaper = ctx.escaper;
                }
                self.use_primary = true;
                self.tcp_notes.clone_from(&ctx.tcp_notes);
                self.route_stats.add_request_passed();
                return ctx.connect_result;
            }
            Ok(Err(_)) => {
                if !Arc::ptr_eq(&self.used_escaper, &self.standby_final_escaper) {
                    if let Some(escaper_stats) = self.standby_final_escaper.get_escape_stats() {
                        escaper_stats.add_https_forward_request_attempted();
                    }
                    self.used_escaper = self.standby_final_escaper.clone();
                }
                self.use_primary = false;
                return match self
                    .used_escaper
                    ._new_https_forward_connection(
                        task_conf,
                        &mut self.tcp_notes,
                        task_notes,
                        task_stats,
                    )
                    .await
                {
                    Ok(c) => {
                        self.route_stats.add_request_passed();
                        Ok(c)
                    }
                    Err(e) => {
                        self.route_stats.add_request_failed();
                        Err(e)
                    }
                };
            }
            Err(_) => {}
        }

        let standby_context = HttpConnectFailoverContext::new(self.standby_final_escaper.clone());
        let standby_task = pin!(standby_context.run_https(task_conf, task_notes, task_stats));

        let ctx = match futures_util::future::select_ok([primary_task, standby_task]).await {
            Ok((ctx, _left)) => {
                self.route_stats.add_request_passed();
                ctx
            }
            Err(ctx) => {
                self.route_stats.add_request_failed();
                ctx
            }
        };
        if !Arc::ptr_eq(&self.used_escaper, &ctx.escaper) {
            if let Some(escaper_stats) = ctx.escaper.get_escape_stats() {
                escaper_stats.add_https_forward_request_attempted();
            }
            self.used_escaper = ctx.escaper;
        }
        self.use_primary = Arc::ptr_eq(&self.used_escaper, &self.primary_final_escaper);
        self.tcp_notes.clone_from(&ctx.tcp_notes);
        ctx.connect_result
    }

    fn save_alive_connection(&mut self, c: BoxHttpForwardConnection) {
        let eof_poller = HttpConnectionEofPoller::spawn(c);
        self.last_connection = Some((Instant::now(), eof_poller));
    }

    fn fetch_tcp_notes(&self, tcp_notes: &mut TcpConnectTaskNotes) {
        tcp_notes.clone_from(&self.tcp_notes);
    }
}
