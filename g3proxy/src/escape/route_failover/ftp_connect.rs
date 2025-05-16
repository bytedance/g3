/*
 * SPDX-License-Identifier: Apache-2.0
 * Copyright 2023-2025 ByteDance and/or its affiliates.
 */

use std::pin::pin;
use std::sync::Arc;

use async_trait::async_trait;

use super::RouteFailoverEscaper;
use crate::escape::ArcEscaper;
use crate::module::ftp_over_http::{
    ArcFtpTaskRemoteControlStats, ArcFtpTaskRemoteTransferStats, BoxFtpConnectContext,
    BoxFtpRemoteConnection, FtpConnectContext, FtpTaskRemoteControlStats,
};
use crate::module::tcp_connect::{TcpConnectError, TcpConnectTaskConf, TcpConnectTaskNotes};
use crate::serve::ServerTaskNotes;

struct NullStats {}

impl FtpTaskRemoteControlStats for NullStats {
    fn add_read_bytes(&self, _size: u64) {}

    fn add_write_bytes(&self, _size: u64) {}
}

struct FtpConnectFailoverContext {
    escaper: ArcEscaper,
}

struct FailoverFtpConnectContext {
    control_connection: Option<BoxFtpRemoteConnection>,
    inner: BoxFtpConnectContext,
}

#[async_trait]
impl FtpConnectContext for FailoverFtpConnectContext {
    async fn new_control_connection(
        &mut self,
        task_conf: &TcpConnectTaskConf<'_>,
        task_notes: &ServerTaskNotes,
        task_stats: ArcFtpTaskRemoteControlStats,
    ) -> Result<BoxFtpRemoteConnection, TcpConnectError> {
        if let Some(c) = self.control_connection.take() {
            return Ok(c);
        }
        self.inner
            .new_control_connection(task_conf, task_notes, task_stats)
            .await
    }

    fn fetch_control_tcp_notes(&self, tcp_notes: &mut TcpConnectTaskNotes) {
        self.inner.fetch_control_tcp_notes(tcp_notes)
    }

    async fn new_transfer_connection(
        &mut self,
        task_conf: &TcpConnectTaskConf<'_>,
        task_notes: &ServerTaskNotes,
        task_stats: ArcFtpTaskRemoteTransferStats,
    ) -> Result<BoxFtpRemoteConnection, TcpConnectError> {
        self.inner
            .new_transfer_connection(task_conf, task_notes, task_stats)
            .await
    }

    fn fetch_transfer_tcp_notes(&self, tcp_notes: &mut TcpConnectTaskNotes) {
        self.inner.fetch_transfer_tcp_notes(tcp_notes)
    }
}

impl FtpConnectFailoverContext {
    fn new(escaper: ArcEscaper) -> Self {
        FtpConnectFailoverContext { escaper }
    }

    async fn run(
        self,
        task_conf: &TcpConnectTaskConf<'_>,
        task_notes: &ServerTaskNotes,
    ) -> Result<FailoverFtpConnectContext, FailoverFtpConnectContext> {
        let mut ftp_ctx = self
            .escaper
            .new_ftp_connect_context(self.escaper.clone(), task_conf, task_notes)
            .await;
        let null_stats = Arc::new(NullStats {});
        // try connect
        match ftp_ctx
            .new_control_connection(task_conf, task_notes, null_stats)
            .await
        {
            Ok(c) => Ok(FailoverFtpConnectContext {
                control_connection: Some(c),
                inner: ftp_ctx,
            }),
            Err(_) => Err(FailoverFtpConnectContext {
                control_connection: None,
                inner: ftp_ctx,
            }),
        }
    }
}

impl RouteFailoverEscaper {
    pub(super) async fn new_ftp_connect_context_with_failover(
        &self,
        task_conf: &TcpConnectTaskConf<'_>,
        task_notes: &ServerTaskNotes,
    ) -> BoxFtpConnectContext {
        let primary_context = FtpConnectFailoverContext::new(self.primary_node.clone());
        let mut primary_task = pin!(primary_context.run(task_conf, task_notes));

        match tokio::time::timeout(self.config.fallback_delay, &mut primary_task).await {
            Ok(Ok(ctx)) => {
                self.stats.add_request_passed();
                return Box::new(ctx);
            }
            Ok(Err(_)) => {
                self.stats.add_request_passed(); // just return the ftp ctx on the standby escaper
                return self
                    .standby_node
                    .new_ftp_connect_context(self.standby_node.clone(), task_conf, task_notes)
                    .await;
            }
            Err(_) => {}
        }

        let standby_context = FtpConnectFailoverContext::new(self.standby_node.clone());
        let standby_task = pin!(standby_context.run(task_conf, task_notes));

        match futures_util::future::select_ok([primary_task, standby_task]).await {
            Ok((ctx, _left)) => {
                self.stats.add_request_passed();
                Box::new(ctx)
            }
            Err(ctx) => {
                self.stats.add_request_failed();
                Box::new(ctx)
            }
        }
    }
}
