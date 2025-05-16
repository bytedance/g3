/*
 * SPDX-License-Identifier: Apache-2.0
 * Copyright 2023-2025 ByteDance and/or its affiliates.
 */

use std::pin::pin;

use anyhow::anyhow;

use g3_daemon::stat::remote::ArcTcpConnectionTaskRemoteStats;

use super::RouteFailoverEscaper;
use crate::audit::AuditContext;
use crate::escape::ArcEscaper;
use crate::module::tcp_connect::{
    TcpConnectError, TcpConnectResult, TcpConnectTaskNotes, TlsConnectTaskConf,
};
use crate::serve::ServerTaskNotes;

struct TlsConnectFailoverContext {
    tcp_notes: TcpConnectTaskNotes,
    audit_ctx: AuditContext,
    connect_result: TcpConnectResult,
}

impl TlsConnectFailoverContext {
    fn new(audit_ctx: &AuditContext) -> Self {
        TlsConnectFailoverContext {
            tcp_notes: TcpConnectTaskNotes::default(),
            audit_ctx: audit_ctx.clone(),
            connect_result: Err(TcpConnectError::EscaperNotUsable(anyhow!(
                "tcp setup connection not called yet"
            ))),
        }
    }

    async fn run(
        mut self,
        escaper: &ArcEscaper,
        task_conf: &TlsConnectTaskConf<'_>,
        task_notes: &ServerTaskNotes,
        task_stats: ArcTcpConnectionTaskRemoteStats,
    ) -> Result<Self, Self> {
        match escaper
            .tls_setup_connection(
                task_conf,
                &mut self.tcp_notes,
                task_notes,
                task_stats,
                &mut self.audit_ctx,
            )
            .await
        {
            Ok(c) => {
                self.connect_result = Ok(c);
                Ok(self)
            }
            Err(e) => {
                self.connect_result = Err(e);
                Ok(self)
            }
        }
    }
}

impl RouteFailoverEscaper {
    pub(super) async fn tls_setup_connection_with_failover(
        &self,
        task_conf: &TlsConnectTaskConf<'_>,
        tcp_notes: &mut TcpConnectTaskNotes,
        task_notes: &ServerTaskNotes,
        task_stats: ArcTcpConnectionTaskRemoteStats,
        audit_ctx: &mut AuditContext,
    ) -> TcpConnectResult {
        let primary_context = TlsConnectFailoverContext::new(audit_ctx);
        let mut primary_task = pin!(primary_context.run(
            &self.primary_node,
            task_conf,
            task_notes,
            task_stats.clone(),
        ));

        match tokio::time::timeout(self.config.fallback_delay, &mut primary_task).await {
            Ok(Ok(ctx)) => {
                self.stats.add_request_passed();
                *audit_ctx = ctx.audit_ctx;
                tcp_notes.clone_from(&ctx.tcp_notes);
                return ctx.connect_result;
            }
            Ok(Err(_)) => {
                return match self
                    .standby_node
                    .tls_setup_connection(task_conf, tcp_notes, task_notes, task_stats, audit_ctx)
                    .await
                {
                    Ok(c) => {
                        self.stats.add_request_passed();
                        Ok(c)
                    }
                    Err(e) => {
                        self.stats.add_request_failed();
                        Err(e)
                    }
                };
            }
            Err(_) => {}
        }

        let standby_context = TlsConnectFailoverContext::new(audit_ctx);
        let standby_task =
            pin!(standby_context.run(&self.standby_node, task_conf, task_notes, task_stats));

        match futures_util::future::select_ok([primary_task, standby_task]).await {
            Ok((ctx, _left)) => {
                self.stats.add_request_passed();
                *audit_ctx = ctx.audit_ctx;
                tcp_notes.clone_from(&ctx.tcp_notes);
                ctx.connect_result
            }
            Err(ctx) => {
                self.stats.add_request_failed();
                *audit_ctx = ctx.audit_ctx;
                tcp_notes.clone_from(&ctx.tcp_notes);
                ctx.connect_result
            }
        }
    }
}
