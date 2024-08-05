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
use tokio::io::{AsyncRead, AsyncWrite};

use g3_imap_proto::command::{Command, ParsedCommand};
use g3_imap_proto::response::{BadResponse, ByeResponse, CommandResult, Response, ServerStatus};
use g3_io_ext::LimitedWriteExt;

use super::{
    CommandLineReceiveExt, ImapInterceptObject, ImapRelayBuf, ResponseAction,
    ResponseLineReceiveExt,
};
use crate::config::server::ServerConfig;
use crate::serve::{ServerTaskError, ServerTaskResult};

enum ClientAction {
    Loop,
    Logout,
    Auth,
    StartTls,
    SendLiteral(usize),
}

pub(super) enum InitiationStatus {
    ServerClose,
    ClientClose,
    StartTls,
    Authenticated,
}

impl<SC> ImapInterceptObject<SC>
where
    SC: ServerConfig + Send + Sync + 'static,
{
    pub(super) async fn relay_not_authenticated<CR, CW, UR, UW>(
        &mut self,
        clt_r: &mut CR,
        clt_w: &mut CW,
        ups_r: &mut UR,
        ups_w: &mut UW,
        relay_buf: &mut ImapRelayBuf,
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
                    if let Some(mut cmd) = self.cmd_pipeline.take_ongoing_command() {
                        self.handle_cmd_continue_line(line, &mut cmd, clt_w, ups_w).await?;
                        if let Some(literal) = cmd.literal_arg {
                            self.cmd_pipeline.set_ongoing_command(cmd);
                            if !literal.wait_continuation {
                                self.relay_client_literal(literal.size, clt_r, ups_w, relay_buf).await?;
                            }
                        } else {
                            self.cmd_pipeline.insert_completed(cmd);
                        }
                    } else {
                        match self.handle_not_authenticated_cmd_line(line, clt_w, ups_w).await? {
                            ClientAction::Auth => {
                                if let Some(status) = self.wait_relay_authenticate_response(
                                    clt_r,
                                    clt_w,
                                    ups_r,
                                    ups_w,
                                    relay_buf,
                                ).await? {
                                    return Ok(status);
                                }
                            }
                            ClientAction::StartTls => {
                                if let Some(status) = self.wait_relay_starttls_response(
                                    clt_w,
                                    ups_r,
                                    relay_buf,
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
                    match self.handle_rsp_line(line, clt_w).await? {
                        ResponseAction::Loop => {}
                        ResponseAction::Close => return Ok(InitiationStatus::ServerClose),
                        ResponseAction::SendLiteral(size) => {
                            self.relay_server_literal(size, clt_w, ups_r,  relay_buf).await?;
                        }
                        ResponseAction::RecvClientLiteral(size) => {
                             self.relay_client_literal(size, clt_r, ups_w, relay_buf).await?;
                        }
                    }
                }
            }
        }
    }

    async fn handle_not_authenticated_cmd_line<CW, UW>(
        &mut self,
        line: &[u8],
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
                    ParsedCommand::Capability | ParsedCommand::NoOperation | ParsedCommand::Id => {
                        self.cmd_pipeline.insert_completed(cmd);
                    }
                    ParsedCommand::Logout => {
                        self.cmd_pipeline.insert_completed(cmd);
                        action = ClientAction::Logout;
                    }
                    ParsedCommand::StartTls => {
                        if self.from_starttls {
                            BadResponse::reply_invalid_command(clt_w, cmd.tag.as_str())
                                .await
                                .map_err(ServerTaskError::ClientTcpWriteFailed)?;
                            return Ok(action);
                        } else {
                            self.cmd_pipeline.insert_completed(cmd);
                        }
                        action = ClientAction::StartTls;
                    }
                    ParsedCommand::Auth => {
                        self.cmd_pipeline.insert_completed(cmd);
                        action = ClientAction::Auth;
                    }
                    ParsedCommand::Login => {
                        if let Some(literal) = cmd.literal_arg {
                            if literal.wait_continuation {
                                action = ClientAction::SendLiteral(literal.size);
                            }
                            self.cmd_pipeline.set_ongoing_command(cmd);
                        } else {
                            self.cmd_pipeline.insert_completed(cmd);
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

    async fn wait_relay_starttls_response<CW, UR>(
        &mut self,
        clt_w: &mut CW,
        ups_r: &mut UR,
        relay_buf: &mut ImapRelayBuf,
    ) -> ServerTaskResult<Option<InitiationStatus>>
    where
        CW: AsyncWrite + Unpin,
        UR: AsyncRead + Unpin,
    {
        loop {
            relay_buf.rsp_recv_buf.consume_line();
            let line = relay_buf.rsp_recv_buf.recv_rsp_line(ups_r).await?;
            let rsp = match Response::parse_line(line) {
                Ok(rsp) => rsp,
                Err(e) => {
                    let _ = ByeResponse::reply_upstream_protocol_error(clt_w).await;
                    return Err(ServerTaskError::UpstreamAppError(anyhow!(
                        "invalid IMAP STARTTLS response line: {e}"
                    )));
                }
            };
            clt_w
                .write_all_flush(line)
                .await
                .map_err(ServerTaskError::ClientTcpWriteFailed)?;
            match rsp {
                Response::CommandResult(r) => {
                    let Some(cmd) = self.cmd_pipeline.remove(&r.tag) else {
                        let _ = ByeResponse::reply_upstream_protocol_error(clt_w).await;
                        return Err(ServerTaskError::UpstreamAppError(anyhow!(
                            "unexpected IMAP command result for tag {}",
                            r.tag
                        )));
                    };
                    if cmd.parsed != ParsedCommand::StartTls {
                        let _ = ByeResponse::reply_upstream_protocol_error(clt_w).await;
                        return Err(ServerTaskError::UpstreamAppError(anyhow!(
                            "unexpected IMAP command result for STARTTLS command"
                        )));
                    }
                    return match r.result {
                        CommandResult::Success => Ok(Some(InitiationStatus::StartTls)),
                        CommandResult::Fail => Ok(None),
                        CommandResult::ProtocolError => Ok(None),
                    };
                }
                Response::ServerStatus(ServerStatus::Close) => {
                    return Ok(Some(InitiationStatus::ServerClose));
                }
                Response::ServerStatus(_s) => {}
                Response::CommandData(_d) => {}
                Response::ContinuationRequest => {
                    let _ = ByeResponse::reply_upstream_protocol_error(clt_w).await;
                    return Err(ServerTaskError::UpstreamAppError(anyhow!(
                        "unexpected IMAP continuation request response for STARTTLS command",
                    )));
                }
            }
        }
    }

    async fn wait_relay_authenticate_response<CR, CW, UR, UW>(
        &mut self,
        clt_r: &mut CR,
        clt_w: &mut CW,
        ups_r: &mut UR,
        ups_w: &mut UW,
        relay_buf: &mut ImapRelayBuf,
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
                    let Some(cmd) = self.cmd_pipeline.remove(&r.tag) else {
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
                        CommandResult::Success => {
                            self.authenticated = true;
                            Ok(Some(InitiationStatus::Authenticated))
                        }
                        CommandResult::Fail => Ok(None),
                        CommandResult::ProtocolError => Ok(None),
                    };
                }
                Response::ServerStatus(ServerStatus::Close) => {
                    return Ok(Some(InitiationStatus::ServerClose));
                }
                Response::ServerStatus(_s) => {}
                Response::CommandData(_d) => {}
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
}
