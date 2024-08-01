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

use g3_imap_proto::command::{Command, CommandPipeline, ParsedCommand};
use g3_imap_proto::response::{BadResponse, ByeResponse, CommandResult, Response, ServerStatus};
use g3_io_ext::{LimitedCopy, LimitedCopyError, LimitedWriteExt};

use super::{CommandLineReceiveExt, ImapRelayBuf, ResponseLineReceiveExt};
use crate::serve::{ServerTaskError, ServerTaskResult};

enum ClientAction {
    Loop,
    Auth,
    Logout,
    SendLiteral(usize),
}

enum ServerAction {
    Loop,
    Return(InitiationStatus),
    SendClientLiteral(usize),
}

pub(super) enum InitiationStatus {
    ServerClose,
    ClientClose,
    StartTls,
    Authenticated,
}

pub(super) struct Initiation {
    from_starttls: bool,
}

impl Initiation {
    pub(super) fn new(from_starttls: bool) -> Self {
        Initiation { from_starttls }
    }

    pub(super) async fn relay<CR, CW, UR, UW>(
        &mut self,
        clt_r: &mut CR,
        clt_w: &mut CW,
        ups_r: &mut UR,
        ups_w: &mut UW,
        relay_buf: &mut ImapRelayBuf,
        cmd_pipeline: &mut CommandPipeline,
    ) -> ServerTaskResult<InitiationStatus>
    where
        CR: AsyncRead + Unpin,
        CW: AsyncWrite + Unpin,
        UR: AsyncRead + Unpin,
        UW: AsyncWrite + Unpin,
    {
        loop {
            relay_buf.cmd_recv_buf.consume_line();
            relay_buf.rsp_recv_buf.consume_line();
            tokio::select! {
                r = relay_buf.cmd_recv_buf.recv_cmd_line(clt_r) => {
                    let line = r?;
                    if let Some(mut cmd) = cmd_pipeline.take_ongoing() {
                        self.handle_cmd_continue_line(line, &mut cmd, clt_w, ups_w).await?;
                        if let Some(literal) = cmd.literal_arg {
                            cmd_pipeline.set_ongoing(cmd);
                            if !literal.wait_continuation {
                                self.relay_client_literal(literal.size, clt_r, ups_w, relay_buf).await?;
                            }
                        } else {
                            cmd_pipeline.insert_completed(cmd);
                        }
                    } else {
                        match self.handle_cmd_line(line, cmd_pipeline, clt_w, ups_w).await? {
                            ClientAction::Auth => {
                                if let Some(status) = self.relay_authenticate_response(
                                    clt_r,
                                    clt_w,
                                    ups_r,
                                    ups_w,
                                    relay_buf,
                                    cmd_pipeline
                                ).await? {
                                    return Ok(status);
                                }
                            }
                            ClientAction::Logout => {
                                return Ok(InitiationStatus::ClientClose);
                            }
                            ClientAction::SendLiteral(size) => {
                                self.relay_client_literal(size, clt_r, ups_w, relay_buf).await?;
                            }
                            ClientAction::Loop => {}
                        }
                    }
                }
                r = relay_buf.rsp_recv_buf.recv_rsp_line(ups_r) => {
                    let line = r?;
                    match self.handle_rsp_line(line, cmd_pipeline, clt_w).await? {
                        ServerAction::Loop => {}
                        ServerAction::Return(status) => return Ok(status),
                        ServerAction::SendClientLiteral(size) => {
                             self.relay_client_literal(size, clt_r, ups_w, relay_buf).await?;
                        }
                    }
                }
            }
        }
    }

    async fn handle_cmd_line<CW, UW>(
        &mut self,
        line: &[u8],
        pipeline: &mut CommandPipeline,
        clt_w: &mut CW,
        ups_w: &mut UW,
    ) -> ServerTaskResult<ClientAction>
    where
        CW: AsyncWrite + Unpin,
        UW: AsyncWrite + Unpin,
    {
        match Command::parse_line(line) {
            Ok(cmd) => {
                let mut action = ClientAction::Loop;
                match cmd.parsed {
                    ParsedCommand::Capability => {
                        pipeline.insert_completed(cmd);
                    }
                    ParsedCommand::NoOperation => {
                        pipeline.insert_completed(cmd);
                    }
                    ParsedCommand::Logout => {
                        pipeline.insert_completed(cmd);
                        action = ClientAction::Logout;
                    }
                    ParsedCommand::StartTls => {
                        if self.from_starttls {
                            BadResponse::reply_invalid_command(clt_w, cmd.tag.as_str())
                                .await
                                .map_err(ServerTaskError::ClientTcpWriteFailed)?;
                            return Ok(action);
                        } else {
                            pipeline.insert_completed(cmd);
                        }
                    }
                    ParsedCommand::Auth => {
                        pipeline.insert_completed(cmd);
                        action = ClientAction::Auth;
                    }
                    ParsedCommand::Login => {
                        if let Some(literal) = cmd.literal_arg {
                            if literal.wait_continuation {
                                action = ClientAction::SendLiteral(literal.size);
                            }
                            pipeline.set_ongoing(cmd);
                        } else {
                            pipeline.insert_completed(cmd);
                        }
                    }
                    _ => {
                        BadResponse::reply_invalid_command(clt_w, cmd.tag.as_str())
                            .await
                            .map_err(ServerTaskError::ClientTcpWriteFailed)?;
                        return Ok(action);
                    }
                }

                ups_w
                    .write_all_flush(line)
                    .await
                    .map_err(ServerTaskError::UpstreamWriteFailed)?;
                Ok(action)
            }
            Err(e) => {
                let _ = ByeResponse::reply_client_protocol_error(clt_w).await;
                Err(ServerTaskError::ClientAppError(anyhow!(
                    "invalid IMAP command line: {e}"
                )))
            }
        }
    }

    async fn handle_cmd_continue_line<CW, UW>(
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

    async fn relay_authenticate_response<CR, CW, UR, UW>(
        &mut self,
        clt_r: &mut CR,
        clt_w: &mut CW,
        ups_r: &mut UR,
        ups_w: &mut UW,
        relay_buf: &mut ImapRelayBuf,
        cmd_pipeline: &mut CommandPipeline,
    ) -> ServerTaskResult<Option<InitiationStatus>>
    where
        CR: AsyncRead + Unpin,
        CW: AsyncWrite + Unpin,
        UR: AsyncRead + Unpin,
        UW: AsyncWrite + Unpin,
    {
        loop {
            relay_buf.rsp_recv_buf.consume_line();
            let line = relay_buf.rsp_recv_buf.recv_rsp_line(ups_r).await?;
            let rsp = match Response::parse_line(line) {
                Ok(rsp) => rsp,
                Err(e) => {
                    let _ = ByeResponse::reply_upstream_protocol_error(clt_w).await;
                    return Err(ServerTaskError::UpstreamAppError(anyhow!(
                        "invalid IMAP AUTHENTICATE response line: {e}"
                    )));
                }
            };
            clt_w
                .write_all_flush(line)
                .await
                .map_err(ServerTaskError::ClientTcpWriteFailed)?;
            match rsp {
                Response::CommandResult(r) => {
                    let Some(cmd) = cmd_pipeline.remove(&r.tag) else {
                        let _ = ByeResponse::reply_upstream_protocol_error(clt_w).await;
                        return Err(ServerTaskError::UpstreamAppError(anyhow!(
                            "unexpected IMAP command result for tag {}",
                            r.tag
                        )));
                    };
                    if cmd.parsed != ParsedCommand::Auth {
                        let _ = ByeResponse::reply_upstream_protocol_error(clt_w).await;
                        return Err(ServerTaskError::UpstreamAppError(anyhow!(
                            "unexpected IMAP command result for command {}",
                            cmd
                        )));
                    }
                    return match r.result {
                        CommandResult::Success => Ok(Some(InitiationStatus::Authenticated)),
                        CommandResult::Fail => Ok(None),
                        CommandResult::ProtocolError => Ok(None),
                    };
                }
                Response::ServerStatus(ServerStatus::Close) => {
                    return Ok(Some(InitiationStatus::ServerClose));
                }
                Response::ServerStatus(_s) => {}
                Response::CommandData(_d) => {
                    // TODO handle CAPABILITY
                }
                Response::ContinuationRequest => {
                    relay_buf.cmd_recv_buf.consume_line();
                    let line = relay_buf.cmd_recv_buf.recv_cmd_line(clt_r).await?;
                    // the client may send a single "*\r\n" to cancel,
                    // but the server is always required to send final response
                    ups_w
                        .write_all_flush(line)
                        .await
                        .map_err(ServerTaskError::UpstreamWriteFailed)?;
                }
            }
        }
    }

    async fn relay_client_literal<CR, UW>(
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

    async fn handle_rsp_line<CW>(
        &mut self,
        line: &[u8],
        cmd_pipeline: &mut CommandPipeline,
        clt_w: &mut CW,
    ) -> ServerTaskResult<ServerAction>
    where
        CW: AsyncWrite + Unpin,
    {
        match Response::parse_line(line) {
            Ok(rsp) => {
                clt_w
                    .write_all_flush(line)
                    .await
                    .map_err(ServerTaskError::ClientTcpWriteFailed)?;
                let mut action = ServerAction::Loop;
                match rsp {
                    Response::CommandResult(r) => {
                        let Some(cmd) = cmd_pipeline.remove(&r.tag) else {
                            let _ = ByeResponse::reply_upstream_protocol_error(clt_w).await;
                            return Err(ServerTaskError::UpstreamAppError(anyhow!(
                                "unexpected IMAP command result for tag {}",
                                r.tag
                            )));
                        };
                        if r.result == CommandResult::Success
                            && cmd.parsed == ParsedCommand::StartTls
                        {
                            action = ServerAction::Return(InitiationStatus::StartTls);
                        }
                    }
                    Response::ServerStatus(ServerStatus::Close) => {
                        action = ServerAction::Return(InitiationStatus::ServerClose);
                    }
                    Response::ServerStatus(_s) => {}
                    Response::CommandData(_d) => {
                        // TODO parse CAPABILITY
                    }
                    Response::ContinuationRequest => {
                        let Some(cmd) = cmd_pipeline.ongoing() else {
                            let _ = ByeResponse::reply_upstream_protocol_error(clt_w).await;
                            return Err(ServerTaskError::UpstreamAppError(anyhow!(
                                "no ongoing IMAP command found when received continuation request"
                            )));
                        };
                        let Some(literal) = cmd.literal_arg else {
                            let _ = ByeResponse::reply_upstream_protocol_error(clt_w).await;
                            return Err(ServerTaskError::UpstreamAppError(anyhow!(
                                "unexpected IMAP continuation request"
                            )));
                        };
                        action = ServerAction::SendClientLiteral(literal.size);
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
