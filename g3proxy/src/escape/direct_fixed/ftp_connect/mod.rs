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

use g3_io_ext::{AggregatedIo, LimitedReader, LimitedWriter};

use super::{DirectFixedEscaper, DirectFixedEscaperStats};
use crate::module::ftp_over_http::{
    ArcFtpTaskRemoteControlStats, ArcFtpTaskRemoteTransferStats, BoxFtpRemoteHttpConnection,
};
use crate::module::tcp_connect::{TcpConnectError, TcpConnectTaskNotes};
use crate::serve::ServerTaskNotes;

mod stats;
use stats::{FtpControlRemoteStats, FtpTransferRemoteStats};

impl DirectFixedEscaper {
    pub(super) async fn new_ftp_control_connection<'a>(
        &'a self,
        tcp_notes: &'a mut TcpConnectTaskNotes,
        task_notes: &'a ServerTaskNotes,
        task_stats: ArcFtpTaskRemoteControlStats,
    ) -> Result<BoxFtpRemoteHttpConnection, TcpConnectError> {
        let stream = self.tcp_connect_to(tcp_notes, task_notes).await?;

        let (r, w) = stream.into_split();

        let mut wrapper_stats = FtpControlRemoteStats::new(&self.stats, task_stats);
        wrapper_stats.push_user_io_stats(self.fetch_user_upstream_io_stats(task_notes));
        let (ups_r_stats, ups_w_stats) = wrapper_stats.into_pair();

        let limit_config = &self.config.general.tcp_sock_speed_limit;
        let r = LimitedReader::new(
            r,
            limit_config.shift_millis,
            limit_config.max_south,
            ups_r_stats,
        );
        let w = LimitedWriter::new(
            w,
            limit_config.shift_millis,
            limit_config.max_north,
            ups_w_stats,
        );

        Ok(Box::new(AggregatedIo {
            reader: r,
            writer: w,
        }))
    }

    pub(super) async fn new_ftp_transfer_connection<'a>(
        &'a self,
        transfer_tcp_notes: &'a mut TcpConnectTaskNotes,
        control_tcp_notes: &'a TcpConnectTaskNotes,
        task_notes: &'a ServerTaskNotes,
        task_stats: ArcFtpTaskRemoteTransferStats,
    ) -> Result<BoxFtpRemoteHttpConnection, TcpConnectError> {
        let stream = self
            .tcp_connect_to_again(transfer_tcp_notes, control_tcp_notes, task_notes)
            .await?;

        let (r, w) = stream.into_split();

        let mut wrapper_stats = FtpTransferRemoteStats::new(&self.stats, task_stats);
        wrapper_stats.push_user_io_stats(self.fetch_user_upstream_io_stats(task_notes));
        let (ups_r_stats, ups_w_stats) = wrapper_stats.into_pair();

        let limit_config = &self.config.general.tcp_sock_speed_limit;
        let r = LimitedReader::new(
            r,
            limit_config.shift_millis,
            limit_config.max_south,
            ups_r_stats,
        );
        let w = LimitedWriter::new(
            w,
            limit_config.shift_millis,
            limit_config.max_north,
            ups_w_stats,
        );

        Ok(Box::new(AggregatedIo {
            reader: r,
            writer: w,
        }))
    }
}
