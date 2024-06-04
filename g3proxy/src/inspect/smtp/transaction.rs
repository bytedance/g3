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

use anyhow::anyhow;
use tokio::io::{AsyncRead, AsyncReadExt, AsyncWrite, AsyncWriteExt};
use tokio::time::Instant;

use g3_io_ext::{LimitedCopy, LimitedCopyError, LimitedWriteExt};
use g3_smtp_proto::command::{Command, MailParam, RecipientParam};
use g3_smtp_proto::io::TextDataReader;
use g3_smtp_proto::response::{ReplyCode, ResponseEncoder, ResponseParser};

use super::{CommandLineRecvExt, ResponseLineRecvExt, ResponseParseExt, SmtpRelayBuf};
use crate::config::server::ServerConfig;
use crate::inspect::StreamInspectContext;
use crate::serve::{ServerTaskError, ServerTaskResult};

pub(super) struct Transaction<'a, SC: ServerConfig> {
    ctx: &'a StreamInspectContext<SC>,
    local_ip: IpAddr,
    allow_chunking: bool,
    allow_burl: bool,
    #[allow(unused)]
    mail_from: MailParam,
    mail_to: Vec<RecipientParam>,
    quit: bool,
}

impl<'a, SC: ServerConfig> Transaction<'a, SC> {
    pub(super) fn new(
        ctx: &'a StreamInspectContext<SC>,
        local_ip: IpAddr,
        allow_chunking: bool,
        allow_burl: bool,
        from: MailParam,
    ) -> Self {
        Transaction {
            ctx,
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
        let mut in_chunking = false;
        loop {
            let mut valid_cmd = Command::NoOperation;

            let Some(cmd_line) = buf
                .cmd_recv_buf
                .recv_cmd_and_relay(
                    clt_r,
                    clt_w,
                    ups_w,
                    |cmd| {
                        match &cmd {
                            Command::Recipient(_) => {}
                            Command::Data => {
                                if in_chunking {
                                    return Some(ResponseEncoder::BAD_SEQUENCE_OF_COMMANDS);
                                }
                            }
                            Command::BinaryData(_) | Command::LastBinaryData(_) => {
                                if !self.allow_chunking {
                                    return Some(ResponseEncoder::COMMAND_NOT_IMPLEMENTED);
                                }
                            }
                            Command::DataByUrl(_) | Command::LastDataByUrl(_) => {
                                if !self.allow_burl {
                                    return Some(ResponseEncoder::COMMAND_NOT_IMPLEMENTED);
                                }
                            }
                            Command::NoOperation => {}
                            Command::Reset => {}
                            Command::Quit => {}
                            _ => return Some(ResponseEncoder::BAD_SEQUENCE_OF_COMMANDS),
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
                Command::Recipient(p) => {
                    let _ = self.recv_relay_rsp(buf, ups_r, clt_w).await?;
                    self.mail_to.push(p);
                }
                Command::Data => {
                    let rsp = self.recv_relay_rsp(buf, ups_r, clt_w).await?;
                    if rsp != ReplyCode::START_MAIL_INPUT {
                        continue;
                    }
                    self.send_txt_data(clt_r, ups_w).await?;
                    let _ = self.recv_relay_rsp(buf, ups_r, clt_w).await?;
                    return Ok(());
                }
                Command::BinaryData(size) => {
                    self.send_bin_data(buf, clt_r, ups_w, size).await?;
                    let _ = self.recv_relay_rsp(buf, ups_r, clt_w).await?;
                    in_chunking = true;
                }
                Command::LastBinaryData(size) => {
                    self.send_bin_data(buf, clt_r, ups_w, size).await?;
                    let _ = self.recv_relay_rsp(buf, ups_r, clt_w).await?;
                    return Ok(());
                }
                Command::DataByUrl(url) => {
                    self.send_burl(ups_w, cmd_line, url).await?;
                    let _ = self.recv_relay_rsp(buf, ups_r, clt_w).await?;
                    in_chunking = true;
                }
                Command::LastDataByUrl(url) => {
                    self.send_burl(ups_w, cmd_line, url).await?;
                    let _ = self.recv_relay_rsp(buf, ups_r, clt_w).await?;
                    return Ok(());
                }
                Command::NoOperation => {
                    let rsp = self.recv_relay_rsp(buf, ups_r, clt_w).await?;
                    if rsp != ReplyCode::OK {
                        return Err(ServerTaskError::UpstreamAppError(anyhow!(
                            "unexpected NOOP reply code {rsp}"
                        )));
                    }
                }
                Command::Reset => {
                    let rsp = self.recv_relay_rsp(buf, ups_r, clt_w).await?;
                    return if rsp != ReplyCode::OK {
                        Err(ServerTaskError::UpstreamAppError(anyhow!(
                            "unexpected RESET reply code {rsp}"
                        )))
                    } else {
                        Ok(())
                    };
                }
                Command::Quit => {
                    let _ = self.recv_relay_rsp(buf, ups_r, clt_w).await?;
                    self.quit = true;
                    return Ok(());
                }
                _ => unreachable!(),
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
            let line = buf
                .rsp_recv_buf
                .read_rsp_line_with_feedback(ups_r, clt_w, self.local_ip)
                .await?;
            let _msg = rsp
                .feed_line_with_feedback(line, clt_w, self.local_ip)
                .await?;

            clt_w
                .write_all_flush(line)
                .await
                .map_err(ServerTaskError::ClientTcpWriteFailed)?;

            if rsp.finished() {
                return Ok(rsp.code());
            }
        }
    }

    async fn send_txt_data<CR, UW>(&self, clt_r: &mut CR, ups_w: &mut UW) -> ServerTaskResult<()>
    where
        CR: AsyncRead + Unpin,
        UW: AsyncWrite + Unpin,
    {
        let mut reader = TextDataReader::new(clt_r);
        self.transfer_data(&mut reader, ups_w).await
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

    async fn send_burl<UW>(
        &self,
        ups_w: &mut UW,
        cmd_line: &[u8],
        _url: String,
    ) -> ServerTaskResult<()>
    where
        UW: AsyncWrite + Unpin,
    {
        ups_w
            .write_all_flush(cmd_line)
            .await
            .map_err(ServerTaskError::UpstreamWriteFailed)?;

        Ok(())
    }
}
