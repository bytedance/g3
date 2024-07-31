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
use tokio::io::{AsyncRead, AsyncWrite, AsyncWriteExt, BufWriter};

use g3_io_ext::{LimitedWriteExt, LineRecvBuf, OnceBufReader, RecvLineError};
use g3_smtp_proto::response::{ReplyCode, ResponseEncoder, ResponseLineError, ResponseParser};
use g3_types::net::Host;

use crate::serve::ServerTaskError;

pub(super) struct Greeting {
    local_ip: IpAddr,
    upstream_host: Host,
    rsp: ResponseParser,
    total_to_write: usize,
}

impl Greeting {
    pub(super) fn new(local_ip: IpAddr) -> Self {
        Greeting {
            local_ip,
            upstream_host: Host::empty(),
            rsp: ResponseParser::default(),
            total_to_write: 0,
        }
    }

    pub(super) fn into_parts(self) -> (ReplyCode, Host) {
        (self.rsp.code(), self.upstream_host)
    }

    async fn do_relay<UR, CW>(
        &mut self,
        mut ups_r: OnceBufReader<UR>,
        clt_w: &mut CW,
    ) -> Result<UR, GreetingError>
    where
        UR: AsyncRead + Unpin,
        CW: AsyncWrite + Unpin,
    {
        let mut recv_buf = LineRecvBuf::<{ ResponseParser::MAX_LINE_SIZE }>::default();

        loop {
            recv_buf.consume_line();
            let line = recv_buf.read_line(&mut ups_r).await?;

            let msg = self.rsp.feed_line(line)?;
            self.total_to_write += line.len();
            clt_w
                .write_all_flush(line)
                .await
                .map_err(GreetingError::ClientWriteFailed)?;

            match self.rsp.code() {
                ReplyCode::SERVICE_READY => {
                    if self.upstream_host.is_empty() {
                        let host_d = match memchr::memchr(b' ', msg) {
                            Some(d) => &msg[..d],
                            None => msg,
                        };
                        if host_d.is_empty() {
                            return Err(GreetingError::NoHostField);
                        }
                        self.upstream_host = Host::parse_smtp_host_address(host_d)
                            .ok_or(GreetingError::UnsupportedHostFormat)?;
                    }
                    if self.rsp.finished() {
                        return Ok(ups_r.into_inner());
                    }
                }
                ReplyCode::NO_SERVICE => {
                    if self.rsp.finished() {
                        return Ok(ups_r.into_inner());
                    }
                }
                c => return Err(GreetingError::UnexpectedReplyCode(c)),
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
        let mut buf_writer = BufWriter::with_capacity(1024, clt_w);
        match tokio::time::timeout(timeout, self.do_relay(ups_r, &mut buf_writer)).await {
            Ok(Ok(ups_r)) => {
                let _ = buf_writer.flush().await;
                Ok(ups_r)
            }
            Ok(Err(e)) => {
                if let GreetingError::ClientWriteFailed(e) = e {
                    Err(GreetingError::ClientWriteFailed(e))
                } else {
                    let _ = buf_writer.flush().await;
                    Err(e)
                }
            }
            Err(_) => {
                let _ = buf_writer.flush().await;
                Err(GreetingError::Timeout)
            }
        }
    }

    pub(super) async fn reply_no_service<CW>(self, e: &GreetingError, clt_w: &mut CW)
    where
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
            GreetingError::UpstreamClosed => "connection closed",
            _ => return,
        };
        let rsp = ResponseEncoder::upstream_service_not_ready(self.local_ip, reason);
        let _ = clt_w.write_all_flush(rsp.as_bytes()).await;
        let _ = clt_w.shutdown().await;
    }
}

#[derive(Debug, Error)]
pub(super) enum GreetingError {
    #[error("greeting timeout")]
    Timeout,
    #[error("invalid greeting response line: {0}")]
    InvalidResponseLine(#[from] ResponseLineError),
    #[error("response line too long")]
    TooLongResponseLine,
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
    #[error("upstream closed connection")]
    UpstreamClosed,
}

impl From<RecvLineError> for GreetingError {
    fn from(value: RecvLineError) -> Self {
        match value {
            RecvLineError::IoError(e) => GreetingError::UpstreamReadFailed(e),
            RecvLineError::IoClosed => GreetingError::UpstreamClosed,
            RecvLineError::Timeout => GreetingError::Timeout,
            RecvLineError::LineTooLong => GreetingError::TooLongResponseLine,
        }
    }
}

impl From<GreetingError> for ServerTaskError {
    fn from(value: GreetingError) -> Self {
        match value {
            GreetingError::Timeout => ServerTaskError::UpstreamAppTimeout("smtp greeting timeout"),
            GreetingError::InvalidResponseLine(e) => {
                ServerTaskError::UpstreamAppError(anyhow!("invalid greeting response line: {e}"))
            }
            GreetingError::TooLongResponseLine => {
                ServerTaskError::UpstreamAppError(anyhow!("response line too long"))
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
            GreetingError::UpstreamClosed => ServerTaskError::ClosedByUpstream,
        }
    }
}
