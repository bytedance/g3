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

use std::time::Duration;

use anyhow::anyhow;
use tokio::io::{AsyncRead, AsyncWrite, AsyncWriteExt};

use g3_io_ext::{LineRecvBuf, RecvLineError};
use g3_smtp_proto::command::Command;
use g3_smtp_proto::response::{ReplyCode, ResponseEncoder, ResponseParser};

use crate::inspect::StreamInspectTaskNotes;
use crate::serve::{ServerTaskError, ServerTaskResult};

pub(super) struct EndQuitServer {}

impl EndQuitServer {
    pub(super) async fn run_to_end<R, W>(
        ups_r: R,
        mut ups_w: W,
        timeout: Duration,
    ) -> ServerTaskResult<()>
    where
        R: AsyncRead + Unpin,
        W: AsyncWrite + Unpin,
    {
        ups_w
            .write_all(b"QUIT\r\n")
            .await
            .map_err(ServerTaskError::UpstreamWriteFailed)?;
        ups_w
            .flush()
            .await
            .map_err(ServerTaskError::UpstreamWriteFailed)?;

        tokio::time::timeout(timeout, EndQuitServer::wait_quit_reply(ups_r))
            .await
            .map_err(|_| {
                ServerTaskError::UpstreamAppTimeout("timeout to wait SMTP QUIT response")
            })?
    }

    async fn wait_quit_reply<R>(mut ups_r: R) -> ServerTaskResult<()>
    where
        R: AsyncRead + Unpin,
    {
        let mut recv_buf = LineRecvBuf::<{ ResponseParser::MAX_LINE_SIZE }>::default();

        let mut rsp = ResponseParser::default();
        loop {
            let line = recv_buf.read_line(&mut ups_r).await.map_err(|e| match e {
                RecvLineError::IoError(e) => ServerTaskError::UpstreamReadFailed(e),
                RecvLineError::IoClosed => ServerTaskError::ClosedByUpstream,
                RecvLineError::LineTooLong => {
                    ServerTaskError::UpstreamAppError(anyhow!("SMTP response line too long"))
                }
            })?;

            rsp.feed_line(line).map_err(|e| {
                ServerTaskError::UpstreamAppError(anyhow!("invalid SMTP QUIT response line: {e}"))
            })?;
            if rsp.code() != ReplyCode::SERVICE_CLOSING {
                return Err(ServerTaskError::UpstreamAppError(anyhow!(
                    "invalid SMTP QUIT response code: {}",
                    rsp.code()
                )));
            }
            if rsp.finished() {
                break;
            }

            let len = line.len();
            recv_buf.consume(len);
        }

        Ok(())
    }
}

pub(super) struct EndWaitClient {}

impl EndWaitClient {
    pub(super) async fn run_to_end<R, W>(
        mut clt_r: R,
        mut clt_w: W,
        task_notes: &StreamInspectTaskNotes,
    ) -> ServerTaskResult<()>
    where
        R: AsyncRead + Unpin,
        W: AsyncWrite + Unpin,
    {
        let mut recv_buf = LineRecvBuf::<{ Command::MAX_LINE_SIZE }>::default();
        loop {
            let line = match recv_buf.read_line(&mut clt_r).await {
                Ok(line) => line,
                Err(e) => {
                    let e = match e {
                        RecvLineError::IoError(e) => ServerTaskError::ClientTcpReadFailed(e),
                        RecvLineError::IoClosed => ServerTaskError::ClosedByClient,
                        RecvLineError::LineTooLong => {
                            let _ = ResponseEncoder::COMMAND_LINE_TOO_LONG
                                .write(&mut clt_w)
                                .await;
                            ServerTaskError::ClientAppError(anyhow!("SMTP command line too long"))
                        }
                    };
                    return Err(e);
                }
            };

            let cmd = match Command::parse_line(line) {
                Ok(cmd) => cmd,
                Err(e) => {
                    let _ = ResponseEncoder::from(&e).write(&mut clt_w).await;
                    return Err(ServerTaskError::ClientAppError(anyhow!(
                        "invalid SMTP command line: {e}"
                    )));
                }
            };
            if cmd == Command::QUIT {
                ResponseEncoder::local_service_closing(task_notes.server_addr.ip())
                    .write(&mut clt_w)
                    .await
                    .map_err(ServerTaskError::ClientTcpWriteFailed)?;

                break;
            } else {
                ResponseEncoder::BAD_SEQUENCE_OF_COMMANDS
                    .write(&mut clt_w)
                    .await
                    .map_err(ServerTaskError::ClientTcpWriteFailed)?;
            }

            let len = line.len();
            recv_buf.consume(len);
        }

        let _ = clt_w.shutdown().await;
        Ok(())
    }
}
