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

use std::os::unix::prelude::AsRawFd;
use std::sync::Arc;
use std::time::Duration;

use log::debug;
use tokio::io::{AsyncRead, AsyncWrite};
use tokio::net::TcpStream;
use tokio_rustls::server::TlsStream;

use g3_daemon::stat::task::TcpStreamTaskStats;
use g3_io_ext::{LimitedReader, LimitedWriter};
use g3_types::net::UpstreamAddr;
use g3_types::route::EgressPathSelection;

use super::common::CommonTaskContext;
use crate::config::server::ServerConfig;
use crate::inspect::StreamInspectContext;
use crate::log::task::tcp_connect::TaskLogForTcpConnect;
use crate::module::tcp_connect::TcpConnectTaskNotes;
use crate::serve::tcp_stream::TcpStreamTaskCltWrapperStats;
use crate::serve::{ServerTaskError, ServerTaskNotes, ServerTaskResult, ServerTaskStage};

pub(super) struct TlsStreamTask {
    ctx: CommonTaskContext,
    tcp_notes: TcpConnectTaskNotes,
    task_notes: ServerTaskNotes,
    task_stats: Arc<TcpStreamTaskStats>,
}

impl TlsStreamTask {
    pub(super) fn new(ctx: CommonTaskContext, upstream: &UpstreamAddr) -> Self {
        let task_notes = ServerTaskNotes::new(
            ctx.worker_id,
            ctx.client_addr,
            ctx.server_addr,
            None,
            Duration::ZERO,
            EgressPathSelection::Default,
        );
        TlsStreamTask {
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

    pub(super) async fn into_running(mut self, stream: TlsStream<TcpStream>) {
        self.pre_start();
        match self.run(stream).await {
            Ok(_) => self
                .get_log_context()
                .log(&self.ctx.task_logger, &ServerTaskError::Finished),
            Err(e) => self.get_log_context().log(&self.ctx.task_logger, &e),
        };
        self.pre_stop();
    }

    fn pre_start(&self) {
        debug!(
            "TlsStream: new client from {} to {} server {}, using escaper {}",
            self.ctx.client_addr,
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

    async fn run(&mut self, clt_stream: TlsStream<TcpStream>) -> ServerTaskResult<()> {
        // set client side socket options
        g3_socket::tcp::set_raw_opts(
            clt_stream.as_raw_fd(),
            &self.ctx.server_config.tcp_misc_opts,
            true,
        )
        .map_err(|_| ServerTaskError::InternalServerError("failed to set client socket options"))?;

        self.task_notes.stage = ServerTaskStage::Connecting;
        let (ups_r, ups_w) = if let Some(tls_client_config) = &self.ctx.tls_client_config {
            if let Some(tls_name) = &self.ctx.server_config.upstream_tls_name {
                self.ctx
                    .escaper
                    .tls_setup_connection(
                        &mut self.tcp_notes,
                        &self.task_notes,
                        self.task_stats.for_escaper(),
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
                        self.task_stats.for_escaper(),
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
                    self.task_stats.for_escaper(),
                )
                .await?
        };

        self.task_notes.stage = ServerTaskStage::Connected;
        self.run_connected(clt_stream, ups_r, ups_w).await
    }

    async fn run_connected<R, W>(
        &mut self,
        clt_stream: TlsStream<TcpStream>,
        ups_r: R,
        ups_w: W,
    ) -> ServerTaskResult<()>
    where
        R: AsyncRead + Send + Sync + Unpin + 'static,
        W: AsyncWrite + Send + Sync + Unpin + 'static,
    {
        self.task_notes.mark_relaying();
        self.relay(clt_stream, ups_r, ups_w).await
    }

    async fn relay<R, W>(
        &mut self,
        clt_stream: TlsStream<TcpStream>,
        ups_r: R,
        ups_w: W,
    ) -> ServerTaskResult<()>
    where
        R: AsyncRead + Send + Sync + Unpin + 'static,
        W: AsyncWrite + Send + Sync + Unpin + 'static,
    {
        let (clt_r, clt_w) = self.split_clt(clt_stream);

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

    fn split_clt(
        &self,
        clt_stream: TlsStream<TcpStream>,
    ) -> (
        LimitedReader<impl AsyncRead>,
        LimitedWriter<impl AsyncWrite>,
    ) {
        let (clt_r, clt_w) = tokio::io::split(clt_stream);

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
