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
use tokio::time::Instant;

use g3_icap_client::reqmod::imap::{
    ImapMessageAdapter, ReqmodAdaptationEndState, ReqmodAdaptationRunState,
};
use g3_imap_proto::command::{Command, ParsedCommand};
use g3_imap_proto::response::{
    BadResponse, ByeResponse, CommandData, CommandResult, Response, ServerStatus, UntaggedResponse,
};
use g3_io_ext::{LimitedCopy, LimitedCopyError, LimitedWriteExt};

use super::{ImapInterceptObject, ImapRelayBuf};
use crate::config::server::ServerConfig;
use crate::serve::{ServerIdleChecker, ServerTaskError, ServerTaskResult};

pub(super) enum ResponseAction {
    Loop,
    Close,
    SendLiteral(u64),
    RecvClientLiteral(u64),
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

    pub(super) async fn relay_client_literal<CR, CW, UW>(
        &mut self,
        literal_size: u64,
        clt_r: &mut CR,
        clt_w: &mut CW,
        ups_w: &mut UW,
        relay_buf: &mut ImapRelayBuf,
    ) -> ServerTaskResult<()>
    where
        CR: AsyncRead + Unpin,
        CW: AsyncWrite + Unpin,
        UW: AsyncWrite + Unpin,
    {
        let Some(cmd) = self.cmd_pipeline.ongoing_command() else {
            return Err(ServerTaskError::InternalServerError(
                "no ongoing IMAP command set while transferring literal data",
            ));
        };

        if cmd.parsed == ParsedCommand::Append {
            if let Some(client) = self.ctx.audit_handle.icap_reqmod_client() {
                match client
                    .imap_message_adaptor(
                        self.ctx.server_config.limited_copy_config(),
                        self.ctx.idle_checker(),
                        literal_size,
                    )
                    .await
                {
                    Ok(adapter) => {
                        return self
                            .relay_append_literal_with_adaptation(
                                literal_size,
                                clt_r,
                                clt_w,
                                ups_w,
                                relay_buf,
                                adapter,
                            )
                            .await;
                    }
                    Err(e) => {
                        if !client.bypass() {
                            return Err(ServerTaskError::InternalAdapterError(e));
                        }
                    }
                }
            }
        }

        relay_buf.cmd_recv_buf.consume_line();
        let cached = relay_buf
            .cmd_recv_buf
            .consume_left(literal_size.min(usize::MAX as u64) as usize);
        ups_w
            .write_all(cached)
            .await
            .map_err(ServerTaskError::UpstreamWriteFailed)?;
        if literal_size > cached.len() as u64 {
            let mut clt_r = clt_r.take(literal_size - cached.len() as u64);

            let idle_duration = self.ctx.server_config.task_idle_check_duration();
            let mut idle_interval =
                tokio::time::interval_at(Instant::now() + idle_duration, idle_duration);
            let mut idle_count = 0;
            let max_idle_count = self.ctx.imap_interception().transfer_max_idle_count;

            let mut clt_to_ups = LimitedCopy::new(&mut clt_r, ups_w, &Default::default());

            loop {
                tokio::select! {
                    biased;

                    r = &mut clt_to_ups => {
                        return match r {
                            Ok(_) => {
                                // ups_w is already flushed
                                Ok(())
                            }
                            Err(LimitedCopyError::ReadFailed(e)) => {
                                let _ = clt_to_ups.write_flush().await;
                                Err(ServerTaskError::ClientTcpReadFailed(e))
                            }
                            Err(LimitedCopyError::WriteFailed(e)) => Err(ServerTaskError::UpstreamWriteFailed(e)),
                        };
                    }
                    _ = idle_interval.tick() => {
                        if clt_to_ups.is_idle() {
                            idle_count += 1;
                            if idle_count >= max_idle_count {
                                return if clt_to_ups.no_cached_data() {
                                    Err(ServerTaskError::ClientAppTimeout("idle while reading literal data"))
                                } else {
                                    Err(ServerTaskError::UpstreamAppTimeout("idle while sending literal data"))
                                };
                            }
                        } else {
                            idle_count = 0;
                            clt_to_ups.reset_active();
                        }

                        if self.ctx.belongs_to_blocked_user() {
                            let _ = clt_to_ups.write_flush().await;
                            return Err(ServerTaskError::CanceledAsUserBlocked);
                        }

                        if self.ctx.server_force_quit() {
                            let _ = clt_to_ups.write_flush().await;
                            return Err(ServerTaskError::CanceledAsServerQuit)
                        }
                    }
                }
            }
        }
        ups_w
            .flush()
            .await
            .map_err(ServerTaskError::UpstreamWriteFailed)
    }

    pub(super) async fn relay_append_literal_with_adaptation<CR, CW, UW>(
        &mut self,
        literal_size: u64,
        clt_r: &mut CR,
        clt_w: &mut CW,
        ups_w: &mut UW,
        relay_buf: &mut ImapRelayBuf,
        mut adapter: ImapMessageAdapter<ServerIdleChecker>,
    ) -> ServerTaskResult<()>
    where
        CR: AsyncRead + Unpin,
        CW: AsyncWrite + Unpin,
        UW: AsyncWrite + Unpin,
    {
        adapter.set_client_addr(self.ctx.task_notes.client_addr);
        if let Some(username) = self.ctx.raw_user_name() {
            adapter.set_client_username(username);
        }

        relay_buf.cmd_recv_buf.consume_line();
        let cached = relay_buf
            .cmd_recv_buf
            .consume_left(literal_size.min(usize::MAX as u64) as usize);

        let mut adaptation_state = ReqmodAdaptationRunState::new(Instant::now());
        match adapter
            .xfer_append(&mut adaptation_state, clt_r, cached, ups_w)
            .await
        {
            Ok(ReqmodAdaptationEndState::OriginalTransferred) => Ok(()),
            Ok(ReqmodAdaptationEndState::AdaptedTransferred) => Ok(()),
            Ok(ReqmodAdaptationEndState::HttpErrResponse(rsp, body)) => {
                if let Some(mut body) = body {
                    let mut body_reader = body.body_reader();
                    let mut sinker = tokio::io::sink();
                    let _ = tokio::io::copy(&mut body_reader, &mut sinker).await;
                    if body_reader.finished() {
                        body.save_connection().await;
                    }
                }
                if let Some(cmd) = self.cmd_pipeline.ongoing_command() {
                    if BadResponse::reply_append_blocked(clt_w, &cmd.tag)
                        .await
                        .is_ok()
                    {
                        let _ = ByeResponse::reply_blocked(clt_w).await;
                    }
                }
                Err(ServerTaskError::InternalAdapterError(anyhow!(
                    "blocked by icap server: {} - {}",
                    rsp.status,
                    rsp.reason
                )))
            }
            Err(e) => Err(e.into()),
        }
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
        literal_size: u64,
        clt_w: &mut CW,
        ups_r: &mut UR,
        relay_buf: &mut ImapRelayBuf,
    ) -> ServerTaskResult<()>
    where
        CW: AsyncWrite + Unpin,
        UR: AsyncRead + Unpin,
    {
        relay_buf.rsp_recv_buf.consume_line();
        let cached = relay_buf
            .rsp_recv_buf
            .consume_left(literal_size.min(usize::MAX as u64) as usize);
        clt_w
            .write_all(cached)
            .await
            .map_err(ServerTaskError::UpstreamWriteFailed)?;
        if literal_size > cached.len() as u64 {
            let mut ups_r = ups_r.take(literal_size - cached.len() as u64);

            let idle_duration = self.ctx.server_config.task_idle_check_duration();
            let mut idle_interval =
                tokio::time::interval_at(Instant::now() + idle_duration, idle_duration);
            let mut idle_count = 0;
            let max_idle_count = self.ctx.imap_interception().transfer_max_idle_count;

            let mut ups_to_clt = LimitedCopy::new(&mut ups_r, clt_w, &Default::default());

            loop {
                tokio::select! {
                    biased;

                    r = &mut ups_to_clt => {
                        return match r {
                            Ok(_) => {
                                // clt_w is already flushed
                                Ok(())
                            }
                            Err(LimitedCopyError::ReadFailed(e)) => {
                                let _ = ups_to_clt.write_flush().await;
                                Err(ServerTaskError::UpstreamReadFailed(e))
                            }
                            Err(LimitedCopyError::WriteFailed(e)) => Err(ServerTaskError::ClientTcpWriteFailed(e)),
                        };
                    }
                    _ = idle_interval.tick() => {
                        if ups_to_clt.is_idle() {
                            idle_count += 1;
                            if idle_count >= max_idle_count {
                                return if ups_to_clt.no_cached_data() {
                                    Err(ServerTaskError::UpstreamAppTimeout("idle while reading literal data"))
                                } else {
                                    Err(ServerTaskError::ClientAppTimeout("idle while sending literal data"))
                                };
                            }
                        } else {
                            idle_count = 0;
                            ups_to_clt.reset_active();
                        }

                        if self.ctx.belongs_to_blocked_user() {
                            let _ = ups_to_clt.write_flush().await;
                            return Err(ServerTaskError::CanceledAsUserBlocked);
                        }

                        if self.ctx.server_force_quit() {
                            let _ = ups_to_clt.write_flush().await;
                            return Err(ServerTaskError::CanceledAsServerQuit)
                        }
                    }
                }
            }
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
