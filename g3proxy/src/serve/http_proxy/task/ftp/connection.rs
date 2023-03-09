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

use async_trait::async_trait;

use g3_ftp_client::FtpConnectionProvider;
use g3_types::net::UpstreamAddr;

use super::FtpOverHttpTaskStats;
use crate::module::ftp_over_http::{
    ArcFtpTaskRemoteControlStats, ArcFtpTaskRemoteTransferStats, BoxFtpConnectContext,
    BoxFtpRemoteHttpConnection,
};
use crate::module::tcp_connect::TcpConnectError;
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
impl FtpConnectionProvider<BoxFtpRemoteHttpConnection, TcpConnectError, ServerTaskNotes>
    for HttpProxyFtpConnectionProvider
{
    async fn new_control_connection(
        &mut self,
        _upstream: &UpstreamAddr,
        task_notes: &ServerTaskNotes,
    ) -> Result<BoxFtpRemoteHttpConnection, TcpConnectError> {
        self.connect_context
            .new_control_connection(
                task_notes,
                Arc::clone(&self.task_stats) as ArcFtpTaskRemoteControlStats,
            )
            .await
    }

    async fn new_data_connection(
        &mut self,
        server_addr: &UpstreamAddr,
        task_notes: &ServerTaskNotes,
    ) -> Result<BoxFtpRemoteHttpConnection, TcpConnectError> {
        self.connect_context
            .new_transfer_connection(
                server_addr,
                task_notes,
                Arc::clone(&self.task_stats) as ArcFtpTaskRemoteTransferStats,
            )
            .await
    }
}
