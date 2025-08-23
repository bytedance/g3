/*
 * SPDX-License-Identifier: Apache-2.0
 * Copyright 2023-2025 ByteDance and/or its affiliates.
 */

use std::sync::Arc;

use g3_io_ext::LimitedStream;
use g3_types::net::UpstreamAddr;

use super::DirectFixedEscaper;
use crate::module::ftp_over_http::{
    ArcFtpTaskRemoteControlStats, ArcFtpTaskRemoteTransferStats, BoxFtpRemoteConnection,
    FtpControlRemoteWrapperStats, FtpTransferRemoteWrapperStats,
};
use crate::module::tcp_connect::{TcpConnectError, TcpConnectTaskConf, TcpConnectTaskNotes};
use crate::serve::ServerTaskNotes;

impl DirectFixedEscaper {
    pub(super) async fn new_ftp_control_connection(
        &self,
        task_conf: &TcpConnectTaskConf<'_>,
        tcp_notes: &mut TcpConnectTaskNotes,
        task_notes: &ServerTaskNotes,
        task_stats: ArcFtpTaskRemoteControlStats,
    ) -> Result<BoxFtpRemoteConnection, TcpConnectError> {
        let mut stream = self
            .tcp_connect_to(task_conf, tcp_notes, task_notes)
            .await?;
        if let Some(version) = self.config.use_proxy_protocol {
            self.send_tcp_proxy_protocol_header(version, &mut stream, task_notes, false)
                .await?;
        }

        let mut wrapper_stats = FtpControlRemoteWrapperStats::new(self.stats.clone(), task_stats);
        wrapper_stats.push_user_io_stats(self.fetch_user_upstream_io_stats(task_notes));
        let wrapper_stats = Arc::new(wrapper_stats);

        let limit_config = &self.config.general.tcp_sock_speed_limit;
        let stream = LimitedStream::local_limited(
            stream,
            limit_config.shift_millis,
            limit_config.max_south,
            limit_config.max_north,
            wrapper_stats,
        );

        Ok(Box::new(stream))
    }

    pub(super) async fn new_ftp_transfer_connection(
        &self,
        task_conf: &TcpConnectTaskConf<'_>,
        transfer_tcp_notes: &mut TcpConnectTaskNotes,
        control_tcp_notes: &TcpConnectTaskNotes,
        task_notes: &ServerTaskNotes,
        task_stats: ArcFtpTaskRemoteTransferStats,
        ftp_server: &UpstreamAddr,
    ) -> Result<BoxFtpRemoteConnection, TcpConnectError> {
        let stream = self
            .tcp_connect_to_again(
                task_conf,
                ftp_server,
                transfer_tcp_notes,
                control_tcp_notes,
                task_notes,
            )
            .await?;

        let mut wrapper_stats = FtpTransferRemoteWrapperStats::new(self.stats.clone(), task_stats);
        wrapper_stats.push_user_io_stats(self.fetch_user_upstream_io_stats(task_notes));
        let wrapper_stats = Arc::new(wrapper_stats);

        let limit_config = &self.config.general.tcp_sock_speed_limit;
        let stream = LimitedStream::local_limited(
            stream,
            limit_config.shift_millis,
            limit_config.max_south,
            limit_config.max_north,
            wrapper_stats,
        );

        Ok(Box::new(stream))
    }
}
