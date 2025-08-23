/*
 * SPDX-License-Identifier: Apache-2.0
 * Copyright 2024-2025 ByteDance and/or its affiliates.
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

    async fn wait_client_logout<CW, UR>(
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

    pub(super) async fn start_server_logout<UR, UW>(
        &mut self,
        ups_r: &mut UR,
        ups_w: &mut UW,
        rsp_recv_buf: &mut LineRecvVec,
    ) where
        UR: AsyncRead + Unpin,
        UW: AsyncWrite + Unpin,
    {
        let _ = ups_w.write_all_flush(b"XXXX LOGOUT\r\n").await;
        let _ = tokio::time::timeout(
            self.ctx.imap_interception().logout_wait_timeout,
            self.wait_server_logout(ups_r, rsp_recv_buf),
        )
        .await;
    }

    async fn wait_server_logout<UR>(&mut self, ups_r: &mut UR, rsp_recv_buf: &mut LineRecvVec)
    where
        UR: AsyncRead + Unpin,
    {
        loop {
            rsp_recv_buf.consume_line();
            let Ok(line) = rsp_recv_buf.recv_rsp_line(ups_r).await else {
                return;
            };
            match Response::parse_line(line) {
                Ok(Response::CommandResult(r)) => {
                    if r.tag.as_bytes() == b"XXXX" {
                        return;
                    }
                    self.cmd_pipeline.remove(&r.tag);
                }
                Ok(Response::ServerStatus(ServerStatus::Close)) => {}
                Ok(_) => {}
                Err(_) => return,
            }
        }
    }
}
