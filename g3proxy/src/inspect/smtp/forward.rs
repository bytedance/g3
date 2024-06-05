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

use tokio::io::{AsyncRead, AsyncWrite};

use g3_dpi::SmtpInterceptionConfig;
use g3_io_ext::{LimitedWriteExt, LineRecvBuf};
use g3_smtp_proto::command::{Command, MailParam};
use g3_smtp_proto::response::{ReplyCode, ResponseEncoder, ResponseParser};

use super::{
    CommandLineRecvExt, InitializedExtensions, Initiation, ResponseLineRecvExt, ResponseParseExt,
    SmtpRelayBuf,
};
use crate::serve::{ServerTaskError, ServerTaskResult};

pub(super) enum ForwardNextAction {
    Quit,
    StartTls,
    ReverseConnection,
    SetExtensions(InitializedExtensions),
    MailTransport(MailParam),
}

pub(super) struct Forward<'a> {
    config: &'a SmtpInterceptionConfig,
    local_ip: IpAddr,
    allow_odmr: bool,
    allow_starttls: bool,
    auth_end: bool,
}

impl<'a> Forward<'a> {
    pub(super) fn new(
        config: &'a SmtpInterceptionConfig,
        local_ip: IpAddr,
        allow_odmr: bool,
        allow_starttls: bool,
    ) -> Self {
        Forward {
            config,
            local_ip,
            allow_odmr,
            allow_starttls,
            auth_end: false,
        }
    }

    pub(super) async fn relay<CR, CW, UR, UW>(
        &mut self,
        buf: &mut SmtpRelayBuf,
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
        loop {
            let mut valid_cmd = Command::NoOperation;
            buf.cmd_recv_buf.consume_line();
            let Some(_cmd_line) = buf
                .cmd_recv_buf
                .recv_cmd_and_relay(
                    self.config.command_wait_timeout,
                    clt_r,
                    clt_w,
                    ups_w,
                    |cmd| {
                        match &cmd {
                            Command::Hello(_)
                            | Command::Recipient(_)
                            | Command::Data
                            | Command::BinaryData(_)
                            | Command::LastBinaryData(_)
                            | Command::DataByUrl(_)
                            | Command::LastDataByUrl(_) => {
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
                Command::Quit => {
                    let _ = self.recv_relay_rsp(buf, ups_r, clt_w).await?;
                    return Ok(ForwardNextAction::Quit);
                }
                Command::StartTls => {
                    let rsp = self.recv_relay_rsp(buf, ups_r, clt_w).await?;
                    if rsp == ReplyCode::SERVICE_READY {
                        return Ok(ForwardNextAction::StartTls);
                    }
                }
                Command::Auth => {
                    self.recv_relay_auth(buf, clt_r, clt_w, ups_r, ups_w)
                        .await?;
                }
                Command::AuthenticatedTurn => {
                    // a max 10min timeout according to RFC2645
                    let rsp = self.recv_relay_rsp(buf, ups_r, clt_w).await?;
                    if rsp == ReplyCode::OK {
                        return Ok(ForwardNextAction::ReverseConnection);
                    }
                }
                Command::ExtendHello(_host) => {
                    let mut initialization = Initiation::new(self.config, self.local_ip, true);
                    if initialization
                        .recv_relay_check_rsp(&mut buf.rsp_recv_buf, ups_r, clt_w)
                        .await?
                        .is_some()
                    {
                        let (_, extensions) = initialization.into_parts();
                        return Ok(ForwardNextAction::SetExtensions(extensions));
                    }
                }
                Command::Mail(param) => {
                    let rsp = self.recv_relay_rsp(buf, ups_r, clt_w).await?;
                    if rsp == ReplyCode::OK {
                        return Ok(ForwardNextAction::MailTransport(param));
                    }
                }
                _ => {
                    self.recv_relay_rsp(buf, ups_r, clt_w).await?;
                }
            }
        }
    }

    async fn recv_relay_rsp<CW, UR>(
        &mut self,
        buf: &mut SmtpRelayBuf,
        ups_r: &mut UR,
        clt_w: &mut CW,
    ) -> ServerTaskResult<ReplyCode>
    where
        CW: AsyncWrite + Unpin,
        UR: AsyncRead + Unpin,
    {
        let mut rsp = ResponseParser::default();
        loop {
            buf.rsp_recv_buf.consume_line();
            let line = buf
                .rsp_recv_buf
                .read_rsp_line_with_feedback(
                    self.config.response_wait_timeout,
                    ups_r,
                    clt_w,
                    self.local_ip,
                )
                .await?;
            let _msg = rsp
                .feed_line_with_feedback(line, clt_w, self.local_ip)
                .await?;

            clt_w
                .write_all_flush(line)
                .await
                .map_err(ServerTaskError::ClientTcpWriteFailed)?;

            if rsp.finished() {
                let code = rsp.code();
                return if code == ReplyCode::SERVICE_NOT_AVAILABLE {
                    Err(ServerTaskError::UpstreamAppUnavailable)
                } else {
                    Ok(code)
                };
            }
        }
    }

    async fn recv_relay_auth<CR, CW, UR, UW>(
        &mut self,
        buf: &mut SmtpRelayBuf,
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
            let rsp = self.recv_relay_rsp(buf, ups_r, clt_w).await?;
            match rsp {
                ReplyCode::AUTHENTICATION_SUCCESSFUL => {
                    self.auth_end = true;
                    return Ok(());
                }
                ReplyCode::AUTH_CONTINUE => {}
                _ => return Ok(()),
            }

            let mut recv_buf = LineRecvBuf::<{ Command::MAX_CONTINUE_LINE_SIZE }>::default();
            match recv_buf
                .read_line_with_timeout(clt_r, self.config.command_wait_timeout)
                .await
            {
                Ok(line) => {
                    ups_w
                        .write_all_flush(line)
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
