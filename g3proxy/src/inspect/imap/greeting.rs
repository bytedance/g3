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

use std::io;
use std::time::Duration;

use anyhow::anyhow;
use thiserror::Error;
use tokio::io::{AsyncRead, AsyncWrite};

use g3_imap_proto::response::{ByeResponse, Response, ResponseLineError, ServerStatus};
use g3_io_ext::{LimitedWriteExt, LineRecvVec, RecvLineError};

use crate::serve::ServerTaskError;

#[derive(Default)]
pub(super) struct Greeting {
    close_service: bool,
    pre_authenticated: bool,
    total_to_write: usize,
}

impl Greeting {
    #[inline]
    pub(super) fn close_service(&self) -> bool {
        self.close_service
    }

    #[inline]
    pub(super) fn pre_authenticated(&self) -> bool {
        self.pre_authenticated
    }

    pub(super) async fn relay<UR, CW>(
        &mut self,
        ups_r: &mut UR,
        clt_w: &mut CW,
        rsp_recv_buf: &mut LineRecvVec,
        rsp_recv_timeout: Duration,
    ) -> Result<(), GreetingError>
    where
        UR: AsyncRead + Unpin,
        CW: AsyncWrite + Unpin,
    {
        let line = rsp_recv_buf
            .read_line_with_timeout(ups_r, rsp_recv_timeout)
            .await?;

        let rsp = Response::parse_line(line)?;
        match rsp {
            Response::ServerStatus(ServerStatus::Close) => {
                self.write_greeting_line(clt_w, line).await?;
                rsp_recv_buf.consume_line();
                self.close_service = true;
                Ok(())
            }
            Response::ServerStatus(ServerStatus::Information) => {
                // TODO parse capability
                self.write_greeting_line(clt_w, line).await?;
                rsp_recv_buf.consume_line();
                Ok(())
            }
            Response::ServerStatus(ServerStatus::Authenticated) => {
                // TODO parse capability
                self.write_greeting_line(clt_w, line).await?;
                rsp_recv_buf.consume_line();
                self.pre_authenticated = true;
                Ok(())
            }
            _ => {
                rsp_recv_buf.consume_line();
                Err(GreetingError::InvalidResponseType)
            }
        }
    }

    async fn write_greeting_line<CW>(
        &mut self,
        clt_w: &mut CW,
        line: &[u8],
    ) -> Result<(), GreetingError>
    where
        CW: AsyncWrite + Unpin,
    {
        self.total_to_write = line.len();
        clt_w
            .write_all_flush(line)
            .await
            .map_err(GreetingError::ClientWriteFailed)?;
        Ok(())
    }

    pub(super) async fn reply_no_service<CW>(self, e: &GreetingError, clt_w: &mut CW)
    where
        CW: AsyncWrite + Unpin,
    {
        if self.total_to_write > 0 {
            return;
        }
        match e {
            GreetingError::Timeout => {
                let _ = ByeResponse::reply_upstream_timeout(clt_w).await;
            }
            GreetingError::InvalidResponseLine(_)
            | GreetingError::TooLongResponseLine
            | GreetingError::InvalidResponseType => {
                let _ = ByeResponse::reply_upstream_protocol_error(clt_w).await;
            }
            GreetingError::ClientWriteFailed(_) => {}
            GreetingError::UpstreamReadFailed(_) | GreetingError::UpstreamClosed => {
                let _ = ByeResponse::reply_upstream_io_error(clt_w).await;
            }
        }
    }
}

#[derive(Debug, Error)]
pub(super) enum GreetingError {
    #[error("greeting timeout")]
    Timeout,
    #[error("invalid greeting response line: {0}")]
    InvalidResponseLine(#[from] ResponseLineError),
    #[error("response line too long")]
    TooLongResponseLine,
    #[error("invalid greeting response type")]
    InvalidResponseType,
    #[error("write to client failed: {0:?}")]
    ClientWriteFailed(io::Error),
    #[error("read from upstream failed: {0:?}")]
    UpstreamReadFailed(io::Error),
    #[error("upstream closed connection")]
    UpstreamClosed,
}

impl From<RecvLineError> for GreetingError {
    fn from(value: RecvLineError) -> Self {
        match value {
            RecvLineError::IoError(e) => GreetingError::UpstreamReadFailed(e),
            RecvLineError::IoClosed => GreetingError::UpstreamClosed,
            RecvLineError::Timeout => GreetingError::Timeout,
            RecvLineError::LineTooLong => GreetingError::TooLongResponseLine,
        }
    }
}

impl From<GreetingError> for ServerTaskError {
    fn from(value: GreetingError) -> Self {
        match value {
            GreetingError::Timeout => ServerTaskError::UpstreamAppTimeout("imap greeting timeout"),
            GreetingError::InvalidResponseLine(e) => {
                ServerTaskError::UpstreamAppError(anyhow!("invalid greeting response line: {e}"))
            }
            GreetingError::TooLongResponseLine => {
                ServerTaskError::UpstreamAppError(anyhow!("response line too long"))
            }
            GreetingError::InvalidResponseType => {
                ServerTaskError::UpstreamAppError(anyhow!("invalid imap greeting response type"))
            }
            GreetingError::ClientWriteFailed(e) => ServerTaskError::ClientTcpWriteFailed(e),
            GreetingError::UpstreamReadFailed(e) => ServerTaskError::UpstreamReadFailed(e),
            GreetingError::UpstreamClosed => ServerTaskError::ClosedByUpstream,
        }
    }
}
