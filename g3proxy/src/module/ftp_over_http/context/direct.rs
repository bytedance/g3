/*
 * SPDX-License-Identifier: Apache-2.0
 * Copyright 2023-2025 ByteDance and/or its affiliates.
 */

use async_trait::async_trait;

use g3_types::net::UpstreamAddr;

use super::{
    ArcFtpTaskRemoteControlStats, ArcFtpTaskRemoteTransferStats, BoxFtpRemoteConnection,
    FtpConnectContext,
};
use crate::escape::ArcEscaper;
use crate::module::tcp_connect::{TcpConnectError, TcpConnectTaskConf, TcpConnectTaskNotes};
use crate::serve::ServerTaskNotes;

pub(crate) struct DirectFtpConnectContext {
    escaper: ArcEscaper,
    upstream: UpstreamAddr,
    control_tcp_notes: TcpConnectTaskNotes,
    transfer_tcp_notes: TcpConnectTaskNotes,
}

impl DirectFtpConnectContext {
    pub(crate) fn new(escaper: ArcEscaper, upstream: UpstreamAddr) -> Self {
        DirectFtpConnectContext {
            escaper,
            upstream,
            control_tcp_notes: TcpConnectTaskNotes::default(),
            transfer_tcp_notes: TcpConnectTaskNotes::default(),
        }
    }
}

#[async_trait]
impl FtpConnectContext for DirectFtpConnectContext {
    async fn new_control_connection(
        &mut self,
        task_conf: &TcpConnectTaskConf<'_>,
        task_notes: &ServerTaskNotes,
        task_stats: ArcFtpTaskRemoteControlStats,
    ) -> Result<BoxFtpRemoteConnection, TcpConnectError> {
        self.escaper
            ._new_ftp_control_connection(
                task_conf,
                &mut self.control_tcp_notes,
                task_notes,
                task_stats,
            )
            .await
    }

    fn fetch_control_tcp_notes(&self, tcp_notes: &mut TcpConnectTaskNotes) {
        tcp_notes.clone_from(&self.control_tcp_notes);
    }

    async fn new_transfer_connection(
        &mut self,
        task_conf: &TcpConnectTaskConf<'_>,
        task_notes: &ServerTaskNotes,
        task_stats: ArcFtpTaskRemoteTransferStats,
    ) -> Result<BoxFtpRemoteConnection, TcpConnectError> {
        self.escaper
            ._new_ftp_transfer_connection(
                task_conf,
                &mut self.transfer_tcp_notes,
                &self.control_tcp_notes,
                task_notes,
                task_stats,
                &self.upstream,
            )
            .await
    }

    fn fetch_transfer_tcp_notes(&self, tcp_notes: &mut TcpConnectTaskNotes) {
        tcp_notes.clone_from(&self.transfer_tcp_notes);
    }
}
