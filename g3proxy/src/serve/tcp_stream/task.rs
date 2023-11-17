/*
 * Copyright 2023 ByteDance and/or its affiliates.
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

use std::sync::Arc;
use std::time::Duration;

use log::debug;
use tokio::io::{AsyncRead, AsyncWrite};

use g3_daemon::stat::task::TcpStreamTaskStats;
use g3_io_ext::{LimitedReader, LimitedWriter};
use g3_types::net::UpstreamAddr;

use super::common::CommonTaskContext;
use super::stats::TcpStreamTaskCltWrapperStats;
use crate::config::server::ServerConfig;
use crate::inspect::StreamInspectContext;
use crate::log::task::tcp_connect::TaskLogForTcpConnect;
use crate::module::tcp_connect::TcpConnectTaskNotes;
use crate::serve::{ServerTaskError, ServerTaskNotes, ServerTaskResult, ServerTaskStage};

pub(super) struct TcpStreamTask {
    ctx: CommonTaskContext,
    tcp_notes: TcpConnectTaskNotes,
    task_notes: ServerTaskNotes,
    task_stats: Arc<TcpStreamTaskStats>,
}

impl TcpStreamTask {
    pub(super) fn new(ctx: CommonTaskContext, upstream: &UpstreamAddr) -> Self {
        let task_notes = ServerTaskNotes::new(ctx.cc_info.clone(), None, Duration::ZERO);
        TcpStreamTask {
            ctx,
            tcp_notes: TcpConnectTaskNotes::new(upstream.clone()),
            task_notes,
            task_stats: Arc::new(TcpStreamTaskStats::default()),
        }
    }

    fn get_log_context(&self) -> TaskLogForTcpConnect {
        TaskLogForTcpConnect {
            task_notes: &self.task_notes,
            tcp_notes: &self.tcp_notes,
            total_time: self.task_notes.time_elapsed(),
            client_rd_bytes: self.task_stats.clt.read.get_bytes(),
            client_wr_bytes: self.task_stats.clt.write.get_bytes(),
            remote_rd_bytes: self.task_stats.ups.read.get_bytes(),
            remote_wr_bytes: self.task_stats.ups.write.get_bytes(),
        }
    }

    pub(super) async fn into_running<CR, CW>(mut self, clt_r: CR, clt_w: CW)
    where
        CR: AsyncRead + Send + Sync + Unpin + 'static,
        CW: AsyncWrite + Send + Sync + Unpin + 'static,
    {
        self.pre_start();
        let (clt_r, clt_w) = self.setup_limit_and_stats(clt_r, clt_w);
        match self.run(clt_r, clt_w).await {
            Ok(_) => self
                .get_log_context()
                .log(&self.ctx.task_logger, &ServerTaskError::Finished),
            Err(e) => self.get_log_context().log(&self.ctx.task_logger, &e),
        };
        self.pre_stop();
    }

    fn pre_start(&self) {
        debug!(
            "TcpStream: new client from {} to {} server {}, using escaper {}",
            self.ctx.client_addr(),
            self.ctx.server_config.server_type(),
            self.ctx.server_config.name(),
            self.ctx.server_config.escaper
        );
        self.ctx.server_stats.add_task();
        self.ctx.server_stats.inc_alive_task();
    }

    fn pre_stop(&self) {
        self.ctx.server_stats.dec_alive_task();
    }

    async fn run<CR, CW>(&mut self, clt_r: CR, clt_w: CW) -> ServerTaskResult<()>
    where
        CR: AsyncRead + Send + Sync + Unpin + 'static,
        CW: AsyncWrite + Send + Sync + Unpin + 'static,
    {
        // set client side socket options
        self.ctx
            .cc_info
            .tcp_sock_set_raw_opts(&self.ctx.server_config.tcp_misc_opts, true)
            .map_err(|_| {
                ServerTaskError::InternalServerError("failed to set client socket options")
            })?;

        self.task_notes.stage = ServerTaskStage::Connecting;
        let (ups_r, ups_w) = if let Some(tls_client_config) = &self.ctx.tls_client_config {
            if let Some(tls_name) = &self.ctx.server_config.upstream_tls_name {
                self.ctx
                    .escaper
                    .tls_setup_connection(
                        &mut self.tcp_notes,
                        &self.task_notes,
                        self.task_stats.clone() as _,
                        tls_client_config,
                        tls_name,
                    )
                    .await?
            } else {
                let tls_name = self.tcp_notes.upstream.host().to_string();
                self.ctx
                    .escaper
                    .tls_setup_connection(
                        &mut self.tcp_notes,
                        &self.task_notes,
                        self.task_stats.clone() as _,
                        tls_client_config,
                        &tls_name,
                    )
                    .await?
            }
        } else {
            self.ctx
                .escaper
                .tcp_setup_connection(
                    &mut self.tcp_notes,
                    &self.task_notes,
                    self.task_stats.clone() as _,
                )
                .await?
        };

        self.task_notes.stage = ServerTaskStage::Connected;
        self.run_connected(clt_r, clt_w, ups_r, ups_w).await
    }

    async fn run_connected<CR, CW, UR, UW>(
        &mut self,
        clt_r: CR,
        clt_w: CW,
        ups_r: UR,
        ups_w: UW,
    ) -> ServerTaskResult<()>
    where
        CR: AsyncRead + Send + Sync + Unpin + 'static,
        CW: AsyncWrite + Send + Sync + Unpin + 'static,
        UR: AsyncRead + Send + Sync + Unpin + 'static,
        UW: AsyncWrite + Send + Sync + Unpin + 'static,
    {
        self.task_notes.mark_relaying();
        self.relay(clt_r, clt_w, ups_r, ups_w).await
    }

    async fn relay<CR, CW, UR, UW>(
        &mut self,
        clt_r: CR,
        clt_w: CW,
        ups_r: UR,
        ups_w: UW,
    ) -> ServerTaskResult<()>
    where
        CR: AsyncRead + Send + Sync + Unpin + 'static,
        CW: AsyncWrite + Send + Sync + Unpin + 'static,
        UR: AsyncRead + Send + Sync + Unpin + 'static,
        UW: AsyncWrite + Send + Sync + Unpin + 'static,
    {
        if let Some(audit_handle) = self.ctx.audit_handle.take() {
            let ctx = StreamInspectContext::new(
                audit_handle,
                self.ctx.server_config.clone(),
                self.ctx.server_stats.clone(),
                self.ctx.server_quit_policy.clone(),
                &self.task_notes,
            );
            crate::inspect::stream::transit_with_inspection(
                clt_r,
                clt_w,
                ups_r,
                ups_w,
                ctx,
                self.tcp_notes.upstream.clone(),
                None,
            )
            .await
        } else {
            crate::inspect::stream::transit_transparent(
                clt_r,
                clt_w,
                ups_r,
                ups_w,
                &self.ctx.server_config,
                &self.ctx.server_quit_policy,
                None,
            )
            .await
        }
    }

    fn setup_limit_and_stats<CR, CW>(
        &self,
        clt_r: CR,
        clt_w: CW,
    ) -> (LimitedReader<CR>, LimitedWriter<CW>)
    where
        CR: AsyncRead,
        CW: AsyncWrite,
    {
        let (clt_r_stats, clt_w_stats) =
            TcpStreamTaskCltWrapperStats::new_pair(&self.ctx.server_stats, &self.task_stats);
        let clt_speed_limit = &self.ctx.server_config.tcp_sock_speed_limit;

        let clt_r = LimitedReader::new(
            clt_r,
            clt_speed_limit.shift_millis,
            clt_speed_limit.max_north,
            clt_r_stats,
        );
        let clt_w = LimitedWriter::new(
            clt_w,
            clt_speed_limit.shift_millis,
            clt_speed_limit.max_south,
            clt_w_stats,
        );

        (clt_r, clt_w)
    }
}
