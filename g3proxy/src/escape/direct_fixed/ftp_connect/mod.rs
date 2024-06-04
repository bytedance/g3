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

use g3_io_ext::LimitedStream;

use super::DirectFixedEscaper;
use crate::module::ftp_over_http::{
    ArcFtpTaskRemoteControlStats, ArcFtpTaskRemoteTransferStats, BoxFtpRemoteConnection,
    FtpControlRemoteWrapperStats, FtpTransferRemoteWrapperStats,
};
use crate::module::tcp_connect::{TcpConnectError, TcpConnectTaskNotes};
use crate::serve::ServerTaskNotes;

impl DirectFixedEscaper {
    pub(super) async fn new_ftp_control_connection<'a>(
        &'a self,
        tcp_notes: &'a mut TcpConnectTaskNotes,
        task_notes: &'a ServerTaskNotes,
        task_stats: ArcFtpTaskRemoteControlStats,
    ) -> Result<BoxFtpRemoteConnection, TcpConnectError> {
        let stream = self.tcp_connect_to(tcp_notes, task_notes).await?;

        let mut wrapper_stats = FtpControlRemoteWrapperStats::new(&self.stats, task_stats);
        wrapper_stats.push_user_io_stats(self.fetch_user_upstream_io_stats(task_notes));
        let wrapper_stats = Arc::new(wrapper_stats);

        let limit_config = &self.config.general.tcp_sock_speed_limit;
        let stream = LimitedStream::new(
            stream,
            limit_config.shift_millis,
            limit_config.max_south,
            limit_config.max_north,
            wrapper_stats,
        );

        Ok(Box::new(stream))
    }

    pub(super) async fn new_ftp_transfer_connection<'a>(
        &'a self,
        transfer_tcp_notes: &'a mut TcpConnectTaskNotes,
        control_tcp_notes: &'a TcpConnectTaskNotes,
        task_notes: &'a ServerTaskNotes,
        task_stats: ArcFtpTaskRemoteTransferStats,
    ) -> Result<BoxFtpRemoteConnection, TcpConnectError> {
        let stream = self
            .tcp_connect_to_again(transfer_tcp_notes, control_tcp_notes, task_notes)
            .await?;

        let mut wrapper_stats = FtpTransferRemoteWrapperStats::new(&self.stats, task_stats);
        wrapper_stats.push_user_io_stats(self.fetch_user_upstream_io_stats(task_notes));
        let wrapper_stats = Arc::new(wrapper_stats);

        let limit_config = &self.config.general.tcp_sock_speed_limit;
        let stream = LimitedStream::new(
            stream,
            limit_config.shift_millis,
            limit_config.max_south,
            limit_config.max_north,
            wrapper_stats,
        );

        Ok(Box::new(stream))
    }
}
