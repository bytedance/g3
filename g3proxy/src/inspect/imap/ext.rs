/*
 * Copyright 2024 ByteDance and/or its affiliates.
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

use tokio::io::AsyncRead;

use g3_io_ext::{LineRecvVec, RecvLineError};

use crate::serve::{ServerTaskError, ServerTaskResult};

pub(super) trait CommandLineReceiveExt {
    async fn recv_cmd_line<'a, CR>(&'a mut self, clt_r: &mut CR) -> ServerTaskResult<&'a [u8]>
    where
        CR: AsyncRead + Unpin;
}

impl CommandLineReceiveExt for LineRecvVec {
    async fn recv_cmd_line<'a, CR>(&'a mut self, clt_r: &mut CR) -> ServerTaskResult<&'a [u8]>
    where
        CR: AsyncRead + Unpin,
    {
        match self.read_line(clt_r).await {
            Ok(line) => Ok(line),
            Err(RecvLineError::Timeout) => Err(ServerTaskError::ClientAppTimeout(
                "timeout to read IMAP command",
            )),
            Err(RecvLineError::IoError(e)) => Err(ServerTaskError::ClientTcpReadFailed(e)),
            Err(RecvLineError::IoClosed) => Err(ServerTaskError::ClosedByClient),
            Err(RecvLineError::LineTooLong) => Err(ServerTaskError::InvalidClientProtocol(
                "too long IMAP command line",
            )),
        }
    }
}

pub(super) trait ResponseLineReceiveExt {
    async fn recv_rsp_line<'a, UR>(&'a mut self, ups_r: &mut UR) -> ServerTaskResult<&'a [u8]>
    where
        UR: AsyncRead + Unpin;
}

impl ResponseLineReceiveExt for LineRecvVec {
    async fn recv_rsp_line<'a, UR>(&'a mut self, ups_r: &mut UR) -> ServerTaskResult<&'a [u8]>
    where
        UR: AsyncRead + Unpin,
    {
        match self.read_line(ups_r).await {
            Ok(line) => Ok(line),
            Err(RecvLineError::Timeout) => Err(ServerTaskError::UpstreamAppTimeout(
                "timeout to read IMAP response",
            )),
            Err(RecvLineError::IoError(e)) => Err(ServerTaskError::UpstreamReadFailed(e)),
            Err(RecvLineError::IoClosed) => Err(ServerTaskError::ClosedByUpstream),
            Err(RecvLineError::LineTooLong) => Err(ServerTaskError::InvalidUpstreamProtocol(
                "too long IMAP response line",
            )),
        }
    }
}
