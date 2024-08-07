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

use anyhow::anyhow;
use tokio::io::{AsyncRead, AsyncReadExt, AsyncWrite, AsyncWriteExt};

use g3_imap_proto::command::{Command, ParsedCommand};
use g3_imap_proto::response::{
    ByeResponse, CommandData, CommandResult, Response, ServerStatus, UntaggedResponse,
};
use g3_io_ext::{LimitedCopy, LimitedCopyError, LimitedWriteExt};

use super::{ImapInterceptObject, ImapRelayBuf};
use crate::config::server::ServerConfig;
use crate::serve::{ServerTaskError, ServerTaskResult};

pub(super) enum ResponseAction {
    Loop,
    Close,
    SendLiteral(usize),
    RecvClientLiteral(usize),
}

impl<SC> ImapInterceptObject<SC>
where
    SC: ServerConfig + Send + Sync + 'static,
{
    pub(super) async fn handle_cmd_continue_line<CW, UW>(
        &mut self,
        line: &[u8],
        cmd: &mut Command,
        clt_w: &mut CW,
        ups_w: &mut UW,
    ) -> ServerTaskResult<()>
    where
        CW: AsyncWrite + Unpin,
        UW: AsyncWrite + Unpin,
    {
        match cmd.parse_continue_line(line) {
            Ok(_) => {
                ups_w
                    .write_all_flush(line)
                    .await
                    .map_err(ServerTaskError::UpstreamWriteFailed)?;
                Ok(())
            }
            Err(e) => {
                let _ = ByeResponse::reply_client_protocol_error(clt_w).await;
                Err(ServerTaskError::ClientAppError(anyhow!(
                    "invalid IMAP command line: {e}"
                )))
            }
        }
    }

    pub(super) async fn relay_client_literal<CR, UW>(
        &mut self,
        literal_size: usize,
        clt_r: &mut CR,
        ups_w: &mut UW,
        relay_buf: &mut ImapRelayBuf,
    ) -> ServerTaskResult<()>
    where
        CR: AsyncRead + Unpin,
        UW: AsyncWrite + Unpin,
    {
        // TODO check for APPEND

        relay_buf.cmd_recv_buf.consume_line();
        let cached = relay_buf.cmd_recv_buf.consume_left(literal_size);
        ups_w
            .write_all(cached)
            .await
            .map_err(ServerTaskError::UpstreamWriteFailed)?;
        if literal_size > cached.len() {
            let mut clt_r = clt_r.take((literal_size - cached.len()) as u64);

            // TODO add timeout limit
            LimitedCopy::new(&mut clt_r, ups_w, &Default::default())
                .await
                .map_err(|e| match e {
                    LimitedCopyError::ReadFailed(e) => ServerTaskError::ClientTcpReadFailed(e),
                    LimitedCopyError::WriteFailed(e) => ServerTaskError::UpstreamWriteFailed(e),
                })?;
        }
        ups_w
            .flush()
            .await
            .map_err(ServerTaskError::UpstreamWriteFailed)
    }

    pub(super) async fn handle_rsp_continue_line<CW>(
        &mut self,
        line: &[u8],
        rsp: &mut UntaggedResponse,
        clt_w: &mut CW,
    ) -> ServerTaskResult<()>
    where
        CW: AsyncWrite + Unpin,
    {
        rsp.parse_continue_line(line).map_err(|e| {
            ServerTaskError::ClientAppError(anyhow!("invalid IMAP command line: {e}"))
        })?;
        clt_w
            .write_all_flush(line)
            .await
            .map_err(ServerTaskError::ClientTcpWriteFailed)?;
        Ok(())
    }

    pub(super) async fn relay_server_literal<CW, UR>(
        &mut self,
        literal_size: usize,
        clt_w: &mut CW,
        ups_r: &mut UR,
        relay_buf: &mut ImapRelayBuf,
    ) -> ServerTaskResult<()>
    where
        CW: AsyncWrite + Unpin,
        UR: AsyncRead + Unpin,
    {
        relay_buf.rsp_recv_buf.consume_line();
        let cached = relay_buf.rsp_recv_buf.consume_left(literal_size);
        clt_w
            .write_all(cached)
            .await
            .map_err(ServerTaskError::UpstreamWriteFailed)?;
        if literal_size > cached.len() {
            let mut ups_r = ups_r.take((literal_size - cached.len()) as u64);

            // TODO add timeout limit
            LimitedCopy::new(&mut ups_r, clt_w, &Default::default())
                .await
                .map_err(|e| match e {
                    LimitedCopyError::ReadFailed(e) => ServerTaskError::UpstreamReadFailed(e),
                    LimitedCopyError::WriteFailed(e) => ServerTaskError::ClientTcpWriteFailed(e),
                })?;
        }
        clt_w
            .flush()
            .await
            .map_err(ServerTaskError::UpstreamWriteFailed)
    }

    pub(super) async fn handle_rsp_line<CW>(
        &mut self,
        line: &[u8],
        clt_w: &mut CW,
    ) -> ServerTaskResult<ResponseAction>
    where
        CW: AsyncWrite + Unpin,
    {
        match Response::parse_line(line) {
            Ok(rsp) => {
                let mut action = ResponseAction::Loop;
                match rsp {
                    Response::CommandResult(r) => {
                        let Some(cmd) = self.cmd_pipeline.remove(&r.tag) else {
                            let _ = ByeResponse::reply_upstream_protocol_error(clt_w).await;
                            return Err(ServerTaskError::UpstreamAppError(anyhow!(
                                "unexpected IMAP command result for tag {}",
                                r.tag
                            )));
                        };
                        clt_w
                            .write_all_flush(line)
                            .await
                            .map_err(ServerTaskError::ClientTcpWriteFailed)?;
                        if r.result == CommandResult::Success {
                            match cmd.parsed {
                                ParsedCommand::Select | ParsedCommand::Examine => {
                                    self.mailbox_selected = true;
                                }
                                ParsedCommand::Close | ParsedCommand::Unselect => {
                                    self.mailbox_selected = false;
                                }
                                _ => {}
                            }
                        }
                    }
                    Response::ServerStatus(ServerStatus::Close) => {
                        clt_w
                            .write_all_flush(line)
                            .await
                            .map_err(ServerTaskError::ClientTcpWriteFailed)?;
                        action = ResponseAction::Close;
                    }
                    Response::ServerStatus(_s) => {
                        clt_w
                            .write_all_flush(line)
                            .await
                            .map_err(ServerTaskError::ClientTcpWriteFailed)?;
                    }
                    Response::CommandData(d) => {
                        match d.command_data {
                            CommandData::Capability => {
                                self.write_capability_response(line, clt_w).await?;
                            }
                            CommandData::Enabled => {
                                self.write_enabled_response(line, clt_w).await?;
                            }
                            _ => {
                                clt_w
                                    .write_all_flush(line)
                                    .await
                                    .map_err(ServerTaskError::ClientTcpWriteFailed)?;
                            }
                        }

                        if let Some(size) = d.literal_data {
                            self.cmd_pipeline.set_ongoing_response(d);
                            action = ResponseAction::SendLiteral(size);
                        }
                    }
                    Response::ContinuationRequest => {
                        let Some(cmd) = self.cmd_pipeline.ongoing_command() else {
                            let _ = ByeResponse::reply_upstream_protocol_error(clt_w).await;
                            return Err(ServerTaskError::UpstreamAppError(anyhow!(
                                "no ongoing IMAP command found when received continuation request"
                            )));
                        };

                        if cmd.parsed == ParsedCommand::Idle {
                            clt_w
                                .write_all_flush(line)
                                .await
                                .map_err(ServerTaskError::ClientTcpWriteFailed)?;
                            return Ok(ResponseAction::Loop);
                        }

                        let Some(literal) = cmd.literal_arg else {
                            let _ = ByeResponse::reply_upstream_protocol_error(clt_w).await;
                            return Err(ServerTaskError::UpstreamAppError(anyhow!(
                                "unexpected IMAP continuation request"
                            )));
                        };

                        clt_w
                            .write_all_flush(line)
                            .await
                            .map_err(ServerTaskError::ClientTcpWriteFailed)?;
                        action = ResponseAction::RecvClientLiteral(literal.size);
                    }
                }
                Ok(action)
            }
            Err(e) => {
                let _ = ByeResponse::reply_upstream_protocol_error(clt_w).await;
                Err(ServerTaskError::UpstreamAppError(anyhow!(
                    "invalid IMAP response line: {e}"
                )))
            }
        }
    }
}
