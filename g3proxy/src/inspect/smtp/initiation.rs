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
use g3_smtp_proto::command::Command;
use g3_smtp_proto::response::{ReplyCode, ResponseParser};
use g3_types::net::Host;

use super::{CommandLineRecvExt, ResponseLineRecvExt, ResponseParseExt};
use crate::serve::{ServerTaskError, ServerTaskResult};

pub struct Initiation {
    local_ip: IpAddr,
    client_host: Host,
}

impl Initiation {
    pub(super) fn new(local_ip: IpAddr) -> Self {
        Initiation {
            local_ip,
            client_host: Host::empty(),
        }
    }

    pub(super) fn into_parts(self) -> Host {
        self.client_host
    }

    pub(super) async fn relay<CR, CW, UR, UW>(
        &mut self,
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
        let mut cmd_recv_buf = LineRecvBuf::<{ Command::MAX_LINE_SIZE }>::default();
        let mut rsp_recv_buf = LineRecvBuf::<{ ResponseParser::MAX_LINE_SIZE }>::default();

        loop {
            let Some(_cmd_line) = cmd_recv_buf
                .recv_cmd_and_relay(
                    clt_r,
                    clt_w,
                    ups_w,
                    |cmd| match cmd {
                        Command::ExtendHello(host) => {
                            self.client_host = host;
                            false
                        }
                        Command::Hello(host) => {
                            self.client_host = host;
                            false
                        }
                        _ => true,
                    },
                    self.local_ip,
                )
                .await?
            else {
                continue;
            };

            if self
                .relay_rsp(&mut rsp_recv_buf, ups_r, clt_w)
                .await?
                .is_some()
            {
                return Ok(());
            }
        }
    }

    async fn relay_rsp<CW, UR>(
        &mut self,
        rsp_recv_buf: &mut LineRecvBuf<{ ResponseParser::MAX_LINE_SIZE }>,
        ups_r: &mut UR,
        clt_w: &mut CW,
    ) -> ServerTaskResult<Option<()>>
    where
        CW: AsyncWrite + Unpin,
        UR: AsyncRead + Unpin,
    {
        let mut rsp = ResponseParser::default();
        loop {
            let line = rsp_recv_buf
                .read_rsp_line_with_feedback(ups_r, clt_w, self.local_ip)
                .await?;
            rsp.feed_line_with_feedback(line, clt_w, self.local_ip)
                .await?;

            if rsp.code() == ReplyCode::OK {
                if rsp.is_first_line() {
                    clt_w
                        .write_all(line)
                        .await
                        .map_err(ServerTaskError::ClientTcpWriteFailed)?;
                } else {
                    // TODO filter out extension
                    clt_w
                        .write_all(line)
                        .await
                        .map_err(ServerTaskError::ClientTcpWriteFailed)?;
                }

                if rsp.finished() {
                    clt_w
                        .flush()
                        .await
                        .map_err(ServerTaskError::ClientTcpWriteFailed)?;
                    return Ok(Some(()));
                }
            } else {
                clt_w
                    .write_all(line)
                    .await
                    .map_err(ServerTaskError::ClientTcpWriteFailed)?;

                if rsp.finished() {
                    clt_w
                        .flush()
                        .await
                        .map_err(ServerTaskError::ClientTcpWriteFailed)?;
                    return Ok(None);
                }
            }
        }
    }
}
