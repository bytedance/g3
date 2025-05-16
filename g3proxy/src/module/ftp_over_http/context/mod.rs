/*
 * SPDX-License-Identifier: Apache-2.0
 * Copyright 2023-2025 ByteDance and/or its affiliates.
 */

use async_trait::async_trait;

use super::{ArcFtpTaskRemoteControlStats, ArcFtpTaskRemoteTransferStats, BoxFtpRemoteConnection};
use crate::module::tcp_connect::{TcpConnectError, TcpConnectTaskConf, TcpConnectTaskNotes};
use crate::serve::ServerTaskNotes;

mod deny;
pub(crate) use deny::DenyFtpConnectContext;

mod direct;
pub(crate) use direct::DirectFtpConnectContext;

#[async_trait]
pub(crate) trait FtpConnectContext {
    async fn new_control_connection(
        &mut self,
        task_conf: &TcpConnectTaskConf<'_>,
        task_notes: &ServerTaskNotes,
        task_stats: ArcFtpTaskRemoteControlStats,
    ) -> Result<BoxFtpRemoteConnection, TcpConnectError>;
    fn fetch_control_tcp_notes(&self, tcp_notes: &mut TcpConnectTaskNotes);

    async fn new_transfer_connection(
        &mut self,
        task_conf: &TcpConnectTaskConf<'_>,
        task_notes: &ServerTaskNotes,
        task_stats: ArcFtpTaskRemoteTransferStats,
    ) -> Result<BoxFtpRemoteConnection, TcpConnectError>;
    fn fetch_transfer_tcp_notes(&self, tcp_notes: &mut TcpConnectTaskNotes);
}

pub(crate) type BoxFtpConnectContext = Box<dyn FtpConnectContext + Send>;
