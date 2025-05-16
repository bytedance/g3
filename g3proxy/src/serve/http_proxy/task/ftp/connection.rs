/*
 * SPDX-License-Identifier: Apache-2.0
 * Copyright 2023-2025 ByteDance and/or its affiliates.
 */

use std::sync::Arc;

use async_trait::async_trait;

use g3_ftp_client::FtpConnectionProvider;
use g3_types::net::UpstreamAddr;

use super::FtpOverHttpTaskStats;
use crate::module::ftp_over_http::{BoxFtpConnectContext, BoxFtpRemoteConnection};
use crate::module::tcp_connect::{TcpConnectError, TcpConnectTaskConf};
use crate::serve::ServerTaskNotes;

pub(super) struct HttpProxyFtpConnectionProvider {
    task_stats: Arc<FtpOverHttpTaskStats>,
    connect_context: BoxFtpConnectContext,
}

impl HttpProxyFtpConnectionProvider {
    pub(super) fn new(
        task_stats: &Arc<FtpOverHttpTaskStats>,
        connect_context: BoxFtpConnectContext,
    ) -> Self {
        HttpProxyFtpConnectionProvider {
            task_stats: Arc::clone(task_stats),
            connect_context,
        }
    }

    #[inline]
    pub(super) fn connect_context(&self) -> &BoxFtpConnectContext {
        &self.connect_context
    }
}

#[async_trait]
impl FtpConnectionProvider<BoxFtpRemoteConnection, TcpConnectError, ServerTaskNotes>
    for HttpProxyFtpConnectionProvider
{
    async fn new_control_connection(
        &mut self,
        upstream: &UpstreamAddr,
        task_notes: &ServerTaskNotes,
    ) -> Result<BoxFtpRemoteConnection, TcpConnectError> {
        let task_conf = TcpConnectTaskConf { upstream };
        self.connect_context
            .new_control_connection(&task_conf, task_notes, self.task_stats.clone())
            .await
    }

    async fn new_data_connection(
        &mut self,
        server_addr: &UpstreamAddr,
        task_notes: &ServerTaskNotes,
    ) -> Result<BoxFtpRemoteConnection, TcpConnectError> {
        let task_conf = TcpConnectTaskConf {
            upstream: server_addr,
        };
        self.connect_context
            .new_transfer_connection(&task_conf, task_notes, self.task_stats.clone())
            .await
    }
}
