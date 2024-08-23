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
use std::time::Duration;

use anyhow::anyhow;
use slog::slog_info;
use tokio::io::{AsyncRead, AsyncReadExt, AsyncWrite, AsyncWriteExt};
use tokio::time::Instant;

use g3_dpi::SmtpInterceptionConfig;
use g3_icap_client::reqmod::mail::{ReqmodAdaptationEndState, ReqmodAdaptationRunState};
use g3_icap_client::reqmod::smtp::SmtpMessageAdapter;
use g3_io_ext::{LimitedCopy, LimitedCopyError, LimitedWriteExt};
use g3_slog_types::LtUuid;
use g3_smtp_proto::command::{Command, MailParam, RecipientParam};
use g3_smtp_proto::io::TextDataReader;
use g3_smtp_proto::response::{ReplyCode, ResponseEncoder, ResponseParser};

use super::{CommandLineRecvExt, ResponseLineRecvExt, ResponseParseExt, SmtpRelayBuf};
use crate::config::server::ServerConfig;
use crate::inspect::StreamInspectContext;
use crate::serve::{ServerIdleChecker, ServerTaskError, ServerTaskResult};

macro_rules! intercept_log {
    ($obj:tt, $($args:tt)+) => {
        slog_info!($obj.ctx.intercept_logger(), $($args)+;
            "intercept_type" => "SmtpTransaction",
            "task_id" => LtUuid($obj.ctx.server_task_id()),
            "depth" => $obj.ctx.inspection_depth,
            "transaction_id" => $obj.transaction_id,
            "mail_from" => $obj.mail_from.reverse_path(),
        )
    };
}

pub(super) struct Transaction<'a, SC: ServerConfig> {
    config: &'a SmtpInterceptionConfig,
    ctx: &'a StreamInspectContext<SC>,
    transaction_id: usize,
    local_ip: IpAddr,
    allow_chunking: bool,
    allow_burl: bool,
    mail_from: MailParam,
    mail_to: Vec<RecipientParam>,
    quit: bool,
}

impl<'a, SC: ServerConfig> Transaction<'a, SC> {
    pub(super) fn new(
        ctx: &'a StreamInspectContext<SC>,
        transaction_id: usize,
        local_ip: IpAddr,
        allow_chunking: bool,
        allow_burl: bool,
        from: MailParam,
    ) -> Self {
        Transaction {
            config: ctx.smtp_interception(),
            ctx,
            transaction_id,
            local_ip,
            allow_chunking,
            allow_burl,
            mail_from: from,
            mail_to: Vec::with_capacity(4),
            quit: false,
        }
    }

    #[inline]
    pub(super) fn quit(&self) -> bool {
        self.quit
    }

    pub(super) async fn relay<CR, CW, UR, UW>(
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
        match self.do_relay(buf, clt_r, clt_w, ups_r, ups_w).await {
            Ok(_) => {
                intercept_log!(self, "finished");
                Ok(())
            }
            Err(e) => {
                intercept_log!(self, "{e}");
                Err(e)
            }
        }
    }

    async fn do_relay<CR, CW, UR, UW>(
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
        let mut in_chunking = false;
        loop {
            buf.cmd_recv_buf.consume_line();
            let (cmd, cmd_line) = buf
                .cmd_recv_buf
                .recv_cmd(self.config.command_wait_timeout, clt_r, clt_w)
                .await?;

            match cmd {
                Command::Recipient(p) => {
                    if in_chunking {
                        self.send_error_to_client(clt_w, ResponseEncoder::BAD_SEQUENCE_OF_COMMANDS)
                            .await?;
                        continue;
                    }
                    self.send_cmd(ups_w, clt_w, cmd_line).await?;
                    let _ = self
                        .recv_relay_rsp(self.config.response_wait_timeout, buf, ups_r, clt_w)
                        .await?;
                    self.mail_to.push(p);
                }
                Command::Data => {
                    if in_chunking {
                        self.send_error_to_client(clt_w, ResponseEncoder::BAD_SEQUENCE_OF_COMMANDS)
                            .await?;
                        continue;
                    }
                    self.send_data_cmd(ups_w, clt_w, cmd_line).await?;
                    let rsp = self
                        .recv_relay_rsp(self.config.data_initiation_timeout, buf, ups_r, clt_w)
                        .await?;
                    if rsp != ReplyCode::START_MAIL_INPUT {
                        continue;
                    }
                    self.send_txt_data(clt_r, clt_w, ups_w).await?;
                    let _ = self
                        .recv_relay_rsp(self.config.data_termination_timeout, buf, ups_r, clt_w)
                        .await?;
                    return Ok(());
                }
                Command::BinaryData(size) => {
                    if !self.allow_chunking {
                        self.send_error_to_client(clt_w, ResponseEncoder::COMMAND_NOT_IMPLEMENTED)
                            .await?;
                        continue;
                    }
                    self.send_bdat_cmd(ups_w, clt_w, cmd_line, size).await?;
                    self.send_bin_data(buf, clt_r, ups_w, size).await?;
                    let _ = self
                        .recv_relay_rsp(self.config.data_termination_timeout, buf, ups_r, clt_w)
                        .await?;
                    in_chunking = true;
                }
                Command::LastBinaryData(size) => {
                    if !self.allow_chunking {
                        self.send_error_to_client(clt_w, ResponseEncoder::COMMAND_NOT_IMPLEMENTED)
                            .await?;
                        continue;
                    }
                    self.send_bdat_cmd(ups_w, clt_w, cmd_line, size).await?;
                    self.send_bin_data(buf, clt_r, ups_w, size).await?;
                    let _ = self
                        .recv_relay_rsp(self.config.data_termination_timeout, buf, ups_r, clt_w)
                        .await?;
                    return Ok(());
                }
                Command::DataByUrl(url) => {
                    if !self.allow_burl || !self.allow_chunking {
                        self.send_error_to_client(clt_w, ResponseEncoder::COMMAND_NOT_IMPLEMENTED)
                            .await?;
                        continue;
                    }
                    self.send_burl_cmd(ups_w, clt_w, cmd_line, url).await?;
                    let _ = self
                        .recv_relay_rsp(self.config.data_termination_timeout, buf, ups_r, clt_w)
                        .await?;
                    in_chunking = true;
                }
                Command::LastDataByUrl(url) => {
                    if !self.allow_burl {
                        self.send_error_to_client(clt_w, ResponseEncoder::COMMAND_NOT_IMPLEMENTED)
                            .await?;
                        continue;
                    }
                    self.send_burl_cmd(ups_w, clt_w, cmd_line, url).await?;
                    let _ = self
                        .recv_relay_rsp(self.config.data_termination_timeout, buf, ups_r, clt_w)
                        .await?;
                    return Ok(());
                }
                Command::NoOperation => {
                    self.send_cmd(ups_w, clt_w, cmd_line).await?;
                    let rsp = self
                        .recv_relay_rsp(self.config.response_wait_timeout, buf, ups_r, clt_w)
                        .await?;
                    if rsp != ReplyCode::OK {
                        return Err(ServerTaskError::UpstreamAppError(anyhow!(
                            "unexpected NOOP reply code {rsp}"
                        )));
                    }
                }
                Command::Reset => {
                    self.send_cmd(ups_w, clt_w, cmd_line).await?;
                    let rsp = self
                        .recv_relay_rsp(self.config.response_wait_timeout, buf, ups_r, clt_w)
                        .await?;
                    return if rsp != ReplyCode::OK {
                        Err(ServerTaskError::UpstreamAppError(anyhow!(
                            "unexpected RESET reply code {rsp}"
                        )))
                    } else {
                        Ok(())
                    };
                }
                Command::Quit => {
                    self.send_cmd(ups_w, clt_w, cmd_line).await?;
                    let _ = self
                        .recv_relay_rsp(self.config.response_wait_timeout, buf, ups_r, clt_w)
                        .await?;
                    self.quit = true;
                    return Ok(());
                }
                _ => {
                    self.send_error_to_client(clt_w, ResponseEncoder::BAD_SEQUENCE_OF_COMMANDS)
                        .await?;
                }
            }
        }
    }

    async fn recv_relay_rsp<CW, UR>(
        &mut self,
        recv_timeout: Duration,
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
                .read_rsp_line_with_feedback(recv_timeout, ups_r, clt_w, self.local_ip)
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

    async fn send_txt_data<CR, CW, UW>(
        &self,
        clt_r: &mut CR,
        clt_w: &mut CW,
        ups_w: &mut UW,
    ) -> ServerTaskResult<()>
    where
        CR: AsyncRead + Unpin,
        CW: AsyncWrite + Unpin,
        UW: AsyncWrite + Unpin,
    {
        if let Some(client) = self.ctx.audit_handle.icap_reqmod_client() {
            match client
                .smtp_message_adaptor(
                    self.ctx.server_config.limited_copy_config(),
                    self.ctx.idle_checker(),
                )
                .await
            {
                Ok(adapter) => {
                    return self
                        .send_txt_data_with_adaptation(clt_r, clt_w, ups_w, adapter)
                        .await;
                }
                Err(e) => {
                    if !client.bypass() {
                        return Err(ServerTaskError::InternalAdapterError(e));
                    }
                }
            }
        }

        let mut reader = TextDataReader::new(clt_r);
        self.transfer_data(&mut reader, ups_w).await
    }

    async fn send_txt_data_with_adaptation<CR, CW, UW>(
        &self,
        clt_r: &mut CR,
        clt_w: &mut CW,
        ups_w: &mut UW,
        mut adapter: SmtpMessageAdapter<ServerIdleChecker>,
    ) -> ServerTaskResult<()>
    where
        CR: AsyncRead + Unpin,
        CW: AsyncWrite + Unpin,
        UW: AsyncWrite + Unpin,
    {
        adapter.set_client_addr(self.ctx.task_notes.client_addr);
        if let Some(username) = self.ctx.raw_user_name() {
            adapter.set_client_username(username.clone());
        }

        let mut adaptation_state = ReqmodAdaptationRunState::new(Instant::now());
        match adapter
            .xfer_data(
                &mut adaptation_state,
                clt_r,
                ups_w,
                &self.mail_from,
                &self.mail_to,
            )
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
                let client_rsp = ResponseEncoder::message_blocked(
                    self.local_ip,
                    format!("ICAP Response {} {}", rsp.status, rsp.reason),
                );
                let _ = client_rsp.write(clt_w).await;
                Err(ServerTaskError::InternalAdapterError(anyhow!(
                    "blocked by icap server: {} - {}",
                    rsp.status,
                    rsp.reason
                )))
            }
            Err(e) => Err(e.into()),
        }
    }

    async fn send_bin_data<CR, UW>(
        &self,
        buf: &mut SmtpRelayBuf,
        clt_r: &mut CR,
        ups_w: &mut UW,
        size: usize,
    ) -> ServerTaskResult<()>
    where
        CR: AsyncRead + Unpin,
        UW: AsyncWrite + Unpin,
    {
        let mut copy_size = size;
        let cache = buf.cmd_recv_buf.consume_left(size);
        if !cache.is_empty() {
            ups_w
                .write_all(cache)
                .await
                .map_err(ServerTaskError::UpstreamWriteFailed)?;
            copy_size -= cache.len();
            if copy_size == 0 {
                ups_w
                    .flush()
                    .await
                    .map_err(ServerTaskError::UpstreamWriteFailed)?;
                return Ok(());
            }
        }

        let mut reader = clt_r.take(copy_size as u64);

        self.transfer_data(&mut reader, ups_w).await
    }

    async fn transfer_data<CR, UW>(&self, clt_r: &mut CR, ups_w: &mut UW) -> ServerTaskResult<()>
    where
        CR: AsyncRead + Unpin,
        UW: AsyncWrite + Unpin,
    {
        let mut clt_to_ups =
            LimitedCopy::new(clt_r, ups_w, &self.ctx.server_config.limited_copy_config());

        let idle_duration = self.ctx.server_config.task_idle_check_duration();
        let mut idle_interval =
            tokio::time::interval_at(Instant::now() + idle_duration, idle_duration);
        let mut idle_count = 0;
        let max_idle_count = self.ctx.task_max_idle_count();

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
                                Err(ServerTaskError::ClientAppTimeout("idle while reading BDAT data"))
                            } else {
                                Err(ServerTaskError::UpstreamAppTimeout("idle while sending BDAT data"))
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

    async fn send_data_cmd<UW, CW>(
        &self,
        ups_w: &mut UW,
        clt_w: &mut CW,
        cmd_line: &[u8],
    ) -> ServerTaskResult<()>
    where
        UW: AsyncWrite + Unpin,
        CW: AsyncWrite + Unpin,
    {
        self.send_cmd(ups_w, clt_w, cmd_line).await
    }

    async fn send_bdat_cmd<UW, CW>(
        &self,
        ups_w: &mut UW,
        clt_w: &mut CW,
        cmd_line: &[u8],
        _size: usize,
    ) -> ServerTaskResult<()>
    where
        UW: AsyncWrite + Unpin,
        CW: AsyncWrite + Unpin,
    {
        self.send_cmd(ups_w, clt_w, cmd_line).await
    }

    async fn send_burl_cmd<UW, CW>(
        &self,
        ups_w: &mut UW,
        clt_w: &mut CW,
        cmd_line: &[u8],
        _url: String,
    ) -> ServerTaskResult<()>
    where
        UW: AsyncWrite + Unpin,
        CW: AsyncWrite + Unpin,
    {
        self.send_cmd(ups_w, clt_w, cmd_line).await
    }

    async fn send_cmd<UW, CW>(
        &self,
        ups_w: &mut UW,
        clt_w: &mut CW,
        cmd_line: &[u8],
    ) -> ServerTaskResult<()>
    where
        UW: AsyncWrite + Unpin,
        CW: AsyncWrite + Unpin,
    {
        match ups_w.write_all_flush(cmd_line).await {
            Ok(_) => Ok(()),
            Err(e) => {
                let _ = ResponseEncoder::upstream_io_error(self.local_ip, &e)
                    .write(clt_w)
                    .await;
                Err(ServerTaskError::UpstreamWriteFailed(e))
            }
        }
    }

    async fn send_error_to_client<W>(
        &self,
        clt_w: &mut W,
        rsp_encoder: ResponseEncoder,
    ) -> ServerTaskResult<()>
    where
        W: AsyncWrite + Unpin,
    {
        rsp_encoder
            .write(clt_w)
            .await
            .map_err(ServerTaskError::ClientTcpWriteFailed)
    }
}
