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

use std::net::IpAddr;

use tokio::io::{AsyncRead, AsyncWrite, AsyncWriteExt};

use g3_io_ext::LineRecvBuf;
use g3_smtp_proto::command::{Command, MailParam};
use g3_smtp_proto::response::{ReplyCode, ResponseEncoder, ResponseParser};

use super::{CommandLineRecvExt, ResponseLineRecvExt, ResponseParseExt};
use crate::serve::{ServerTaskError, ServerTaskResult};

pub(super) enum ForwardNextAction {
    StartTls,
    ReverseConnection,
    MailTransport(MailParam),
}

pub(super) struct Forward {
    local_ip: IpAddr,
    allow_odmr: bool,
    allow_starttls: bool,
    auth_end: bool,
}

impl Forward {
    pub(super) fn new(local_ip: IpAddr, allow_odmr: bool, allow_starttls: bool) -> Self {
        Forward {
            local_ip,
            allow_odmr,
            allow_starttls,
            auth_end: false,
        }
    }

    pub(super) async fn relay<CR, CW, UR, UW>(
        &mut self,
        clt_r: &mut CR,
        clt_w: &mut CW,
        ups_r: &mut UR,
        ups_w: &mut UW,
    ) -> ServerTaskResult<ForwardNextAction>
    where
        CR: AsyncRead + Unpin,
        CW: AsyncWrite + Unpin,
        UR: AsyncRead + Unpin,
        UW: AsyncWrite + Unpin,
    {
        let mut cmd_recv_buf = LineRecvBuf::<{ Command::MAX_LINE_SIZE }>::default();
        let mut rsp_recv_buf = LineRecvBuf::<{ ResponseParser::MAX_LINE_SIZE }>::default();

        loop {
            let mut valid_cmd = Command::NoOperation;
            let Some(_cmd_line) = cmd_recv_buf
                .recv_cmd_and_relay(
                    clt_r,
                    clt_w,
                    ups_w,
                    |cmd| {
                        match &cmd {
                            Command::Hello(_)
                            | Command::ExtendHello(_)
                            | Command::Recipient(_)
                            | Command::Data
                            | Command::DataByUrl(_)
                            | Command::BinaryData(_)
                            | Command::LastBinaryData(_) => {
                                return Some(ResponseEncoder::BAD_SEQUENCE_OF_COMMANDS)
                            }
                            Command::Auth => {
                                if self.auth_end {
                                    return Some(ResponseEncoder::BAD_SEQUENCE_OF_COMMANDS);
                                }
                            }
                            Command::AuthenticatedTurn => {
                                if !self.allow_odmr {
                                    return Some(ResponseEncoder::COMMAND_NOT_IMPLEMENTED);
                                }
                                if !self.auth_end {
                                    return Some(ResponseEncoder::AUTHENTICATION_REQUIRED);
                                }
                            }
                            Command::StartTls => {
                                if !self.allow_starttls {
                                    return Some(ResponseEncoder::COMMAND_NOT_IMPLEMENTED);
                                }
                            }
                            _ => {}
                        };
                        valid_cmd = cmd;
                        None
                    },
                    self.local_ip,
                )
                .await?
            else {
                continue;
            };

            match valid_cmd {
                Command::StartTls => {
                    let rsp = self.recv_relay_rsp(&mut rsp_recv_buf, ups_r, clt_w).await?;
                    if rsp == ReplyCode::SERVICE_READY {
                        return Ok(ForwardNextAction::StartTls);
                    }
                }
                Command::Auth => {
                    self.recv_relay_auth(&mut rsp_recv_buf, clt_r, clt_w, ups_r, ups_w)
                        .await?;
                }
                Command::AuthenticatedTurn => {
                    // a max 10min timeout according to RFC2645
                    let rsp = self.recv_relay_rsp(&mut rsp_recv_buf, ups_r, clt_w).await?;
                    if rsp == ReplyCode::OK {
                        return Ok(ForwardNextAction::ReverseConnection);
                    }
                }
                Command::Mail(param) => {
                    let rsp = self.recv_relay_rsp(&mut rsp_recv_buf, ups_r, clt_w).await?;
                    if rsp == ReplyCode::OK {
                        return Ok(ForwardNextAction::MailTransport(param));
                    }
                }
                _ => {
                    self.recv_relay_rsp(&mut rsp_recv_buf, ups_r, clt_w).await?;
                }
            }
        }
    }

    async fn recv_relay_rsp<CW, UR>(
        &mut self,
        rsp_recv_buf: &mut LineRecvBuf<{ ResponseParser::MAX_LINE_SIZE }>,
        ups_r: &mut UR,
        clt_w: &mut CW,
    ) -> ServerTaskResult<ReplyCode>
    where
        CW: AsyncWrite + Unpin,
        UR: AsyncRead + Unpin,
    {
        let mut rsp = ResponseParser::default();
        loop {
            let line = rsp_recv_buf
                .read_rsp_line_with_feedback(ups_r, clt_w, self.local_ip)
                .await?;
            let _msg = rsp
                .feed_line_with_feedback(line, clt_w, self.local_ip)
                .await?;

            clt_w
                .write_all(line)
                .await
                .map_err(ServerTaskError::ClientTcpWriteFailed)?;

            if rsp.finished() {
                return Ok(rsp.code());
            }
        }
    }

    async fn recv_relay_auth<CR, CW, UR, UW>(
        &mut self,
        rsp_recv_buf: &mut LineRecvBuf<{ ResponseParser::MAX_LINE_SIZE }>,
        clt_r: &mut CR,
        clt_w: &mut CW,
        ups_r: &mut UR,
        ups_w: &mut UW,
    ) -> ServerTaskResult<()>
    where
        CR: AsyncRead + Unpin,
        CW: AsyncWrite + Unpin,
        UR: AsyncRead + Unpin,
        UW: AsyncWrite + Unpin,
    {
        loop {
            let rsp = self.recv_relay_rsp(rsp_recv_buf, ups_r, clt_w).await?;
            match rsp {
                ReplyCode::AUTHENTICATION_SUCCESSFUL => {
                    self.auth_end = true;
                    return Ok(());
                }
                ReplyCode::AUTH_CONTINUE => {}
                _ => return Ok(()),
            }

            let mut recv_buf = LineRecvBuf::<{ Command::MAX_CONTINUE_LINE_SIZE }>::default();
            match recv_buf.read_line(clt_r).await {
                Ok(line) => {
                    ups_w
                        .write_all(line)
                        .await
                        .map_err(ServerTaskError::UpstreamWriteFailed)?;
                    recv_buf.consume_line();
                }
                Err(e) => {
                    let e = LineRecvBuf::<{ Command::MAX_CONTINUE_LINE_SIZE }>::handle_line_error(
                        e, clt_w,
                    )
                    .await;
                    return Err(e);
                }
            }
        }
    }
}
