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

use g3_types::net::UpstreamAddr;

use super::{
    ArcFtpTaskRemoteControlStats, ArcFtpTaskRemoteTransferStats, BoxFtpRemoteHttpConnection,
    FtpConnectContext,
};
use crate::module::tcp_connect::{TcpConnectError, TcpConnectTaskNotes};
use crate::serve::ServerTaskNotes;

pub(crate) struct DenyFtpConnectContext {
    escaper_name: String,
    control_error: Option<TcpConnectError>,
}

impl DenyFtpConnectContext {
    pub(crate) fn new(escaper_name: &str, error: Option<TcpConnectError>) -> Self {
        DenyFtpConnectContext {
            escaper_name: escaper_name.to_string(),
            control_error: error,
        }
    }
}

#[async_trait]
impl FtpConnectContext for DenyFtpConnectContext {
    async fn new_control_connection(
        &mut self,
        _task_notes: &ServerTaskNotes,
        _task_stats: ArcFtpTaskRemoteControlStats,
    ) -> Result<BoxFtpRemoteHttpConnection, TcpConnectError> {
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
        _server_addr: &UpstreamAddr,
        _task_notes: &ServerTaskNotes,
        _task_stats: ArcFtpTaskRemoteTransferStats,
    ) -> Result<BoxFtpRemoteHttpConnection, TcpConnectError> {
        Err(TcpConnectError::MethodUnavailable)
    }

    fn fetch_transfer_tcp_notes(&self, tcp_notes: &mut TcpConnectTaskNotes) {
        tcp_notes.escaper.clone_from(&self.escaper_name)
    }
}
