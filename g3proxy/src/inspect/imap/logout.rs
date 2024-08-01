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

use g3_imap_proto::command::ParsedCommand;
use g3_imap_proto::response::{Response, ServerStatus};
use g3_io_ext::{LimitedWriteExt, LineRecvVec};

use super::{ImapInterceptObject, ResponseLineReceiveExt};
use crate::config::server::ServerConfig;
use crate::serve::{ServerTaskError, ServerTaskResult};

impl<SC> ImapInterceptObject<SC>
where
    SC: ServerConfig + Send + Sync + 'static,
{
    pub(super) async fn handle_client_logout<CW, UR>(
        &mut self,
        clt_w: &mut CW,
        ups_r: &mut UR,
        rsp_recv_buf: &mut LineRecvVec,
    ) -> ServerTaskResult<()>
    where
        CW: AsyncWrite + Unpin,
        UR: AsyncRead + Unpin,
    {
        self.client_logout = true;

        tokio::time::timeout(
            self.ctx.imap_interception().logout_wait_timeout,
            self.wait_client_logout(clt_w, ups_r, rsp_recv_buf),
        )
        .await
        .map_err(|_| ServerTaskError::UpstreamAppTimeout("timeout to wait IMAP LOGOUT response"))?
    }

    pub(super) async fn wait_client_logout<CW, UR>(
        &mut self,
        clt_w: &mut CW,
        ups_r: &mut UR,
        rsp_recv_buf: &mut LineRecvVec,
    ) -> ServerTaskResult<()>
    where
        CW: AsyncWrite + Unpin,
        UR: AsyncRead + Unpin,
    {
        loop {
            rsp_recv_buf.consume_line();
            let line = rsp_recv_buf.recv_rsp_line(ups_r).await?;
            clt_w
                .write_all_flush(line)
                .await
                .map_err(ServerTaskError::ClientTcpWriteFailed)?;
            match Response::parse_line(line) {
                Ok(Response::CommandResult(r)) => {
                    let Some(cmd) = self.cmd_pipeline.remove(&r.tag) else {
                        continue;
                    };
                    if cmd.parsed == ParsedCommand::Logout {
                        return Ok(());
                    }
                }
                Ok(Response::ServerStatus(ServerStatus::Close)) => {
                    self.server_bye = true;
                }
                Ok(_) => {}
                Err(e) => {
                    return Err(ServerTaskError::UpstreamAppError(anyhow!(
                        "invalid IMAP response line: {e}"
                    )));
                }
            }
        }
    }
}
