/*
 * SPDX-License-Identifier: Apache-2.0
 * Copyright 2023-2025 ByteDance and/or its affiliates.
 */

use async_trait::async_trait;

use g3_types::metrics::NodeName;

use super::{
    ArcFtpTaskRemoteControlStats, ArcFtpTaskRemoteTransferStats, BoxFtpRemoteConnection,
    FtpConnectContext,
};
use crate::module::tcp_connect::{TcpConnectError, TcpConnectTaskConf, TcpConnectTaskNotes};
use crate::serve::ServerTaskNotes;

pub(crate) struct DenyFtpConnectContext {
    escaper_name: NodeName,
    control_error: Option<TcpConnectError>,
}

impl DenyFtpConnectContext {
    pub(crate) fn new(escaper_name: &NodeName, error: Option<TcpConnectError>) -> Self {
        DenyFtpConnectContext {
            escaper_name: escaper_name.clone(),
            control_error: error,
        }
    }
}

#[async_trait]
impl FtpConnectContext for DenyFtpConnectContext {
    async fn new_control_connection(
        &mut self,
        _task_conf: &TcpConnectTaskConf<'_>,
        _task_notes: &ServerTaskNotes,
        _task_stats: ArcFtpTaskRemoteControlStats,
    ) -> Result<BoxFtpRemoteConnection, TcpConnectError> {
        if let Some(e) = self.control_error.take() {
            Err(e)
        } else {
            Err(TcpConnectError::MethodUnavailable)
        }
    }

    fn fetch_control_tcp_notes(&self, tcp_notes: &mut TcpConnectTaskNotes) {
        tcp_notes.escaper.clone_from(&self.escaper_name)
    }

    async fn new_transfer_connection(
        &mut self,
        _task_conf: &TcpConnectTaskConf<'_>,
        _task_notes: &ServerTaskNotes,
        _task_stats: ArcFtpTaskRemoteTransferStats,
    ) -> Result<BoxFtpRemoteConnection, TcpConnectError> {
        Err(TcpConnectError::MethodUnavailable)
    }

    fn fetch_transfer_tcp_notes(&self, tcp_notes: &mut TcpConnectTaskNotes) {
        tcp_notes.escaper.clone_from(&self.escaper_name)
    }
}
