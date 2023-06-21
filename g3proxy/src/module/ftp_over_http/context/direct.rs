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
    ArcFtpTaskRemoteControlStats, ArcFtpTaskRemoteTransferStats, BoxFtpRemoteConnection,
    FtpConnectContext,
};
use crate::escape::ArcEscaper;
use crate::module::tcp_connect::{TcpConnectError, TcpConnectTaskNotes};
use crate::serve::ServerTaskNotes;

pub(crate) struct DirectFtpConnectContextParam {}

pub(crate) struct DirectFtpConnectContext {
    escaper: ArcEscaper,
    control_tcp_notes: TcpConnectTaskNotes,
    transfer_tcp_notes: TcpConnectTaskNotes,
}

impl DirectFtpConnectContext {
    pub(crate) fn new(escaper: ArcEscaper, upstream: UpstreamAddr) -> Self {
        DirectFtpConnectContext {
            escaper,
            control_tcp_notes: TcpConnectTaskNotes::new(upstream),
            transfer_tcp_notes: TcpConnectTaskNotes::empty(),
        }
    }
}

#[async_trait]
impl FtpConnectContext for DirectFtpConnectContext {
    async fn new_control_connection(
        &mut self,
        task_notes: &ServerTaskNotes,
        task_stats: ArcFtpTaskRemoteControlStats,
    ) -> Result<BoxFtpRemoteConnection, TcpConnectError> {
        self.escaper
            ._new_ftp_control_connection(&mut self.control_tcp_notes, task_notes, task_stats)
            .await
    }

    fn fetch_control_tcp_notes(&self, tcp_notes: &mut TcpConnectTaskNotes) {
        tcp_notes.fill_generated(&self.control_tcp_notes);
    }

    async fn new_transfer_connection(
        &mut self,
        server_addr: &UpstreamAddr,
        task_notes: &ServerTaskNotes,
        task_stats: ArcFtpTaskRemoteTransferStats,
    ) -> Result<BoxFtpRemoteConnection, TcpConnectError> {
        let param = DirectFtpConnectContextParam {};
        self.transfer_tcp_notes.upstream.clone_from(server_addr);
        self.escaper
            ._new_ftp_transfer_connection(
                &mut self.transfer_tcp_notes,
                &self.control_tcp_notes,
                task_notes,
                task_stats,
                Box::new(param),
            )
            .await
    }

    fn fetch_transfer_tcp_notes(&self, tcp_notes: &mut TcpConnectTaskNotes) {
        tcp_notes
            .upstream
            .clone_from(&self.transfer_tcp_notes.upstream);
        tcp_notes.fill_generated(&self.transfer_tcp_notes);
    }
}
