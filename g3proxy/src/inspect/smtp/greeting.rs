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
use thiserror::Error;
use tokio::io::{AsyncRead, AsyncReadExt, AsyncWrite, AsyncWriteExt};

use g3_io_ext::OnceBufReader;
use g3_smtp_proto::response::{ReplyCode, Response, ResponseLineError};
use g3_types::net::Host;

use crate::inspect::StreamInspectTaskNotes;
use crate::serve::ServerTaskError;

pub(super) struct Greeting {
    host: Host,
    rsp: Response,
    end: bool,
    total_to_write: usize,
}

impl Default for Greeting {
    fn default() -> Self {
        Greeting::new()
    }
}

impl Greeting {
    pub(super) fn new() -> Self {
        Greeting {
            host: Host::empty(),
            rsp: Response::default(),
            end: false,
            total_to_write: 0,
        }
    }

    pub(super) fn into_parts(self) -> (ReplyCode, Host) {
        (self.rsp.code(), self.host)
    }

    async fn relay_buf<CW>(&mut self, buf: &[u8], clt_w: &mut CW) -> Result<usize, GreetingError>
    where
        CW: AsyncWrite + Unpin,
    {
        let mut offset = 0usize;
        while offset < buf.len() {
            if let Some(d) = memchr::memchr(b'\n', &buf[offset..]) {
                let line = &buf[offset..=offset + d];
                let msg = self.rsp.feed_line(&buf[offset..=offset + d])?;
                self.total_to_write += line.len();
                clt_w
                    .write_all(line)
                    .await
                    .map_err(GreetingError::ClientWriteFailed)?;
                offset += line.len();
                match self.rsp.code() {
                    ReplyCode::SERVICE_READY => {
                        if self.host.is_empty() {
                            let host_d = match memchr::memchr(b' ', msg) {
                                Some(d) => &msg[..d],
                                None => msg,
                            };
                            if host_d.is_empty() {
                                return Err(GreetingError::NoHostField);
                            }
                            self.host = Host::parse_smtp_host_address(host_d)
                                .ok_or(GreetingError::UnsupportedHostFormat)?;
                        }
                        if self.rsp.finished() {
                            self.end = true;
                            return Ok(offset);
                        }
                    }
                    ReplyCode::NO_SERVICE => {
                        if self.rsp.finished() {
                            self.end = true;
                            return Ok(offset);
                        }
                    }
                    c => return Err(GreetingError::UnexpectedReplyCode(c)),
                }
            }
        }
        Ok(offset)
    }

    pub(super) async fn do_relay<UR, CW>(
        &mut self,
        mut ups_r: OnceBufReader<UR>,
        clt_w: &mut CW,
    ) -> Result<UR, GreetingError>
    where
        UR: AsyncRead + Unpin,
        CW: AsyncWrite + Unpin,
    {
        let mut offset = 0;
        let mut recv_buf = [0u8; Response::MAX_LINE_SIZE];

        if let Some(buf) = ups_r.take_buf() {
            let nw = self.relay_buf(&buf, clt_w).await?;
            if self.end {
                return Ok(ups_r.into_inner());
            }
            if nw < buf.len() {
                recv_buf.copy_from_slice(&buf[nw..]);
                offset = buf.len() - nw;
            }
        }

        let mut ups_r = ups_r.into_inner();
        loop {
            let mut b = &mut recv_buf[offset..];
            let nr = ups_r
                .read_buf(&mut b)
                .await
                .map_err(GreetingError::UpstreamReadFailed)?;
            let len = offset + nr;
            let nw = self.relay_buf(&recv_buf[..len], clt_w).await?;
            if self.end {
                return Ok(ups_r);
            }
            if nw < len {
                recv_buf.copy_within(nw..len, 0);
                offset = len - nw;
            } else {
                offset = 0;
            }
        }
    }

    pub(super) async fn relay<UR, CW>(
        &mut self,
        ups_r: OnceBufReader<UR>,
        clt_w: &mut CW,
        timeout: Duration,
    ) -> Result<UR, GreetingError>
    where
        UR: AsyncRead + Unpin,
        CW: AsyncWrite + Unpin,
    {
        tokio::time::timeout(timeout, self.do_relay(ups_r, clt_w))
            .await
            .map_err(|_| GreetingError::Timeout)?
    }

    pub(super) async fn reply_no_service<CW>(
        &self,
        e: &GreetingError,
        clt_w: &mut CW,
        task_notes: &StreamInspectTaskNotes,
    ) where
        CW: AsyncWrite + Unpin,
    {
        if self.total_to_write > 0 {
            return;
        }
        let reason = match e {
            GreetingError::Timeout => "read timeout",
            GreetingError::InvalidResponseLine(_) => "invalid response",
            GreetingError::UnexpectedReplyCode(_) => "unexpected reply code",
            GreetingError::UpstreamReadFailed(_) => "read failed",
            _ => return,
        };
        let msg = match task_notes.server_addr.ip() {
            IpAddr::V4(v4) => format!("554 [{v4}] upstream service not ready - {reason}\r\n"),
            IpAddr::V6(v6) => format!("554 Ipv6:{v6} upstream service not ready - {reason}\r\n"),
        };
        let _ = clt_w.write_all(msg.as_bytes()).await;
    }
}

#[derive(Debug, Error)]
pub(super) enum GreetingError {
    #[error("greeting timeout")]
    Timeout,
    #[error("invalid greeting response line: {0}")]
    InvalidResponseLine(#[from] ResponseLineError),
    #[error("unexpected reply code {0} in greeting stage")]
    UnexpectedReplyCode(ReplyCode),
    #[error("no host field in greeting message")]
    NoHostField,
    #[error("unsupported host format")]
    UnsupportedHostFormat,
    #[error("write to client failed: {0:?}")]
    ClientWriteFailed(io::Error),
    #[error("read from upstream failed: {0:?}")]
    UpstreamReadFailed(io::Error),
}

impl From<GreetingError> for ServerTaskError {
    fn from(value: GreetingError) -> Self {
        match value {
            GreetingError::Timeout => ServerTaskError::UpstreamAppTimeout("smtp greeting timeout"),
            GreetingError::InvalidResponseLine(e) => {
                ServerTaskError::UpstreamAppError(anyhow!("invalid greeting response line: {e}"))
            }
            GreetingError::UnexpectedReplyCode(c) => ServerTaskError::UpstreamAppError(anyhow!(
                "unknown reply code {c} in greeting stage",
            )),
            GreetingError::NoHostField => {
                ServerTaskError::UpstreamAppError(anyhow!("no host found in smtp greeting message"))
            }
            GreetingError::UnsupportedHostFormat => ServerTaskError::UpstreamAppError(anyhow!(
                "unsupported host in smtp greeting message"
            )),
            GreetingError::ClientWriteFailed(e) => ServerTaskError::ClientTcpWriteFailed(e),
            GreetingError::UpstreamReadFailed(e) => ServerTaskError::UpstreamReadFailed(e),
        }
    }
}
