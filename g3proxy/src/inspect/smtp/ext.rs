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
use std::net::IpAddr;
use std::time::Duration;

use anyhow::anyhow;
use tokio::io::{AsyncRead, AsyncWrite};

use g3_io_ext::{LimitedWriteExt, LineRecvBuf, RecvLineError};
use g3_smtp_proto::command::Command;
use g3_smtp_proto::response::{ResponseEncoder, ResponseParser};

use crate::serve::{ServerTaskError, ServerTaskResult};

pub(super) trait ResponseLineRecvExt {
    async fn read_rsp_line_with_feedback<'a, R, W>(
        &'a mut self,
        recv_timeout: Duration,
        ups_r: &mut R,
        clt_w: &mut W,
        local_ip: IpAddr,
    ) -> ServerTaskResult<&[u8]>
    where
        R: AsyncRead + Unpin,
        W: AsyncWrite + Unpin;
}

impl<const MAX_LINE_SIZE: usize> ResponseLineRecvExt for LineRecvBuf<MAX_LINE_SIZE> {
    async fn read_rsp_line_with_feedback<'a, R, W>(
        &'a mut self,
        recv_timeout: Duration,
        ups_r: &mut R,
        clt_w: &mut W,
        local_ip: IpAddr,
    ) -> ServerTaskResult<&[u8]>
    where
        R: AsyncRead + Unpin,
        W: AsyncWrite + Unpin,
    {
        match self.read_line_with_timeout(ups_r, recv_timeout).await {
            Ok(line) => Ok(line),
            Err(e) => match e {
                RecvLineError::IoError(e) => {
                    let _ = ResponseEncoder::upstream_io_error(local_ip, &e)
                        .write(clt_w)
                        .await;
                    Err(ServerTaskError::UpstreamReadFailed(e))
                }
                RecvLineError::IoClosed => {
                    let _ = ResponseEncoder::upstream_io_closed(local_ip)
                        .write(clt_w)
                        .await;
                    Err(ServerTaskError::ClosedByUpstream)
                }
                RecvLineError::Timeout => {
                    let _ = ResponseEncoder::local_service_not_available(local_ip)
                        .write(clt_w)
                        .await;
                    Err(ServerTaskError::UpstreamAppTimeout(
                        "timeout to get upstream response",
                    ))
                }
                RecvLineError::LineTooLong => {
                    let _ = ResponseEncoder::upstream_line_too_long(local_ip)
                        .write(clt_w)
                        .await;
                    Err(ServerTaskError::UpstreamAppError(anyhow!(
                        "SMTP response line too long"
                    )))
                }
            },
        }
    }
}

pub(super) trait ResponseParseExt {
    async fn feed_line_with_feedback<'a, W>(
        &mut self,
        line: &'a [u8],
        clt_w: &mut W,
        local_ip: IpAddr,
    ) -> ServerTaskResult<&'a [u8]>
    where
        W: AsyncWrite + Unpin;
}

impl ResponseParseExt for ResponseParser {
    async fn feed_line_with_feedback<'a, W>(
        &mut self,
        line: &'a [u8],
        clt_w: &mut W,
        local_ip: IpAddr,
    ) -> ServerTaskResult<&'a [u8]>
    where
        W: AsyncWrite + Unpin,
    {
        match self.feed_line(line) {
            Ok(msg) => Ok(msg),
            Err(e) => {
                let _ = ResponseEncoder::upstream_response_error(local_ip, &e)
                    .write(clt_w)
                    .await;
                Err(ServerTaskError::UpstreamAppError(anyhow!(
                    "invalid SMTP QUIT response line: {e}"
                )))
            }
        }
    }
}

pub(super) trait CommandLineRecvExt {
    async fn recv_cmd_and_relay<CR, CW, UW, F>(
        &mut self,
        recv_timeout: Duration,
        clt_r: &mut CR,
        clt_w: &mut CW,
        ups_w: &mut UW,
        is_bad_sequence: F,
        local_ip: IpAddr,
    ) -> ServerTaskResult<Option<&[u8]>>
    where
        CR: AsyncRead + Unpin,
        CW: AsyncWrite + Unpin,
        UW: AsyncWrite + Unpin,
        F: FnMut(Command) -> Option<ResponseEncoder>;

    async fn handle_line_error<CW>(e: RecvLineError, clt_w: &mut CW) -> ServerTaskError
    where
        CW: AsyncWrite + Unpin,
    {
        match e {
            RecvLineError::IoError(e) => ServerTaskError::ClientTcpReadFailed(e),
            RecvLineError::IoClosed => ServerTaskError::ClosedByClient,
            RecvLineError::Timeout => {
                ServerTaskError::ClientAppTimeout("timeout to wait client command")
            }
            RecvLineError::LineTooLong => {
                let _ = ResponseEncoder::COMMAND_LINE_TOO_LONG.write(clt_w).await;
                ServerTaskError::ClientAppError(anyhow!("SMTP command line too long"))
            }
        }
    }
}

impl<const MAX_LINE_SIZE: usize> CommandLineRecvExt for LineRecvBuf<MAX_LINE_SIZE> {
    async fn recv_cmd_and_relay<CR, CW, UW, F>(
        &mut self,
        recv_timeout: Duration,
        clt_r: &mut CR,
        clt_w: &mut CW,
        ups_w: &mut UW,
        mut check_cmd: F,
        local_ip: IpAddr,
    ) -> ServerTaskResult<Option<&[u8]>>
    where
        CR: AsyncRead + Unpin,
        CW: AsyncWrite + Unpin,
        UW: AsyncWrite + Unpin,
        F: FnMut(Command) -> Option<ResponseEncoder>,
    {
        match self.read_line_with_timeout(clt_r, recv_timeout).await {
            Ok(line) => match Command::parse_line(line) {
                Ok(cmd) => {
                    let is_burl = matches!(cmd, Command::DataByUrl(_) | Command::LastDataByUrl(_));
                    if let Some(rsp_encoder) = check_cmd(cmd) {
                        rsp_encoder
                            .write(clt_w)
                            .await
                            .map_err(ServerTaskError::ClientTcpWriteFailed)?;
                        Ok(None)
                    } else if is_burl {
                        // DO NOT send BURL line out, we will do it later
                        Ok(Some(line))
                    } else if let Err(e) = send_cmd(ups_w, line).await {
                        let _ = ResponseEncoder::upstream_io_error(local_ip, &e)
                            .write(clt_w)
                            .await;
                        Err(ServerTaskError::UpstreamReadFailed(e))
                    } else {
                        Ok(Some(line))
                    }
                }
                Err(e) => {
                    let _ = ResponseEncoder::from(&e).write(clt_w).await;
                    Err(ServerTaskError::ClientAppError(anyhow!(
                        "invalid SMTP command line: {e}"
                    )))
                }
            },
            Err(e) => {
                let e = Self::handle_line_error(e, clt_w).await;
                Err(e)
            }
        }
    }
}

async fn send_cmd<W>(ups_w: &mut W, line: &[u8]) -> io::Result<()>
where
    W: AsyncWrite + Unpin,
{
    ups_w.write_all_flush(line).await
}
