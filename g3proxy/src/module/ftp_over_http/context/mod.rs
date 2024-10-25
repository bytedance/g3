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
