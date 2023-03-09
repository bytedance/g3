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

use bytes::BytesMut;
use log::debug;
use tokio::io::{AsyncRead, AsyncWrite};

use g3_daemon::stat::task::{TcpStreamConnectionStats, TcpStreamTaskStats};
use g3_dpi::Protocol;
use g3_io_ext::{FlexBufReader, LimitedCopy, LimitedReader, LimitedWriter, OnceBufReader};
use g3_types::net::UpstreamAddr;
use g3_types::route::EgressPathSelection;

use super::CommonTaskContext;
use crate::config::server::ServerConfig;
use crate::inspect::{StreamInspectContext, StreamInspection};
use crate::log::task::tcp_connect::TaskLogForTcpConnect;
use crate::module::tcp_connect::TcpConnectTaskNotes;
use crate::serve::tcp_stream::TcpStreamTaskCltWrapperStats;
use crate::serve::{ServerTaskError, ServerTaskNotes, ServerTaskResult, ServerTaskStage};

pub(crate) struct TcpStreamTask {
    ctx: CommonTaskContext,
    protocol: Protocol,
    tcp_notes: TcpConnectTaskNotes,
    task_notes: ServerTaskNotes,
    task_stats: Arc<TcpStreamTaskStats>,
}

impl TcpStreamTask {
    pub(crate) fn new(
        ctx: CommonTaskContext,
        protocol: Protocol,
        upstream: UpstreamAddr,
        wait_time: Duration,
        pre_handshake_stats: TcpStreamConnectionStats,
    ) -> Self {
        let task_notes = ServerTaskNotes::new(
            ctx.worker_id,
            ctx.client_addr,
            ctx.server_addr,
            None,
            wait_time,
            EgressPathSelection::Default,
        );
        TcpStreamTask {
            ctx,
            protocol,
            tcp_notes: TcpConnectTaskNotes::new(upstream),
            task_notes,
            task_stats: Arc::new(TcpStreamTaskStats::with_clt_stats(pre_handshake_stats)),
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

    pub(crate) async fn into_running<R, W>(
        mut self,
        clt_r: LimitedReader<R>,
        clt_r_buf: BytesMut,
        clt_w: LimitedWriter<W>,
    ) where
        R: AsyncRead + Send + Sync + Unpin + 'static,
        W: AsyncWrite + Send + Sync + Unpin + 'static,
    {
        self.pre_start();
        match self.run(clt_r, clt_r_buf, clt_w).await {
            Ok(_) => self
                .get_log_context()
                .log(&self.ctx.task_logger, &ServerTaskError::Finished),
            Err(e) => self.get_log_context().log(&self.ctx.task_logger, &e),
        };
        self.pre_stop();
    }

    fn pre_start(&self) {
        debug!(
            "SniProxy: new client from {} to {} server {}, using escaper {}",
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

    async fn run<R, W>(
        &mut self,
        clt_r: LimitedReader<R>,
        clt_r_buf: BytesMut,
        clt_w: LimitedWriter<W>,
    ) -> ServerTaskResult<()>
    where
        R: AsyncRead + Send + Sync + Unpin + 'static,
        W: AsyncWrite + Send + Sync + Unpin + 'static,
    {
        self.task_stats.clt.read.add_bytes(clt_r_buf.len() as u64);

        // set client side socket options
        g3_socket::tcp::set_raw_opts(
            self.ctx.tcp_client_socket,
            &self.ctx.server_config.tcp_misc_opts,
            true,
        )
        .map_err(|_| ServerTaskError::InternalServerError("failed to set client socket options"))?;

        self.task_notes.stage = ServerTaskStage::Connecting;
        let (ups_r, ups_w) = self
            .ctx
            .escaper
            .tcp_setup_connection(
                &mut self.tcp_notes,
                &self.task_notes,
                self.task_stats.for_escaper(),
            )
            .await?;

        self.task_notes.stage = ServerTaskStage::Connected;
        self.run_connected(clt_r, clt_r_buf, clt_w, ups_r, ups_w)
            .await
    }

    async fn run_connected<CR, CW, UR, UW>(
        &mut self,
        clt_r: LimitedReader<CR>,
        clt_r_buf: BytesMut,
        clt_w: LimitedWriter<CW>,
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
        self.relay(clt_r, clt_r_buf, clt_w, ups_r, ups_w).await
    }

    async fn relay<CR, CW, UR, UW>(
        &mut self,
        mut clt_r: LimitedReader<CR>,
        clt_r_buf: BytesMut,
        mut clt_w: LimitedWriter<CW>,
        mut ups_r: UR,
        mut ups_w: UW,
    ) -> ServerTaskResult<()>
    where
        CR: AsyncRead + Send + Sync + Unpin + 'static,
        CW: AsyncWrite + Send + Sync + Unpin + 'static,
        UR: AsyncRead + Send + Sync + Unpin + 'static,
        UW: AsyncWrite + Send + Sync + Unpin + 'static,
    {
        let (clt_r_stats, clt_w_stats) =
            TcpStreamTaskCltWrapperStats::new_pair(&self.ctx.server_stats, &self.task_stats);
        clt_r.reset_stats(clt_r_stats);
        clt_w.reset_stats(clt_w_stats);

        if let Some(audit_handle) = self.ctx.audit_handle.take() {
            let ctx = StreamInspectContext::new(
                audit_handle,
                self.ctx.server_config.clone(),
                self.ctx.server_stats.clone(),
                self.ctx.server_quit_policy.clone(),
                &self.task_notes,
            );
            let protocol_inspector = ctx.protocol_inspector(None);
            match self.protocol {
                Protocol::TlsModern => {
                    if let Some(tls_interception) = ctx.tls_interception() {
                        let mut tls_obj = crate::inspect::tls::TlsInterceptObject::new(
                            ctx,
                            self.tcp_notes.upstream.clone(),
                            tls_interception,
                        );
                        tls_obj.set_io(
                            OnceBufReader::new(Box::new(clt_r), clt_r_buf),
                            Box::new(clt_w),
                            Box::new(ups_r),
                            Box::new(ups_w),
                        );
                        return StreamInspection::TlsModern(tls_obj)
                            .into_loop_inspection(protocol_inspector)
                            .await;
                    }
                }
                Protocol::Http1 => {
                    let mut h1_obj = crate::inspect::http::H1InterceptObject::new(ctx);
                    h1_obj.set_io(
                        FlexBufReader::with_bytes(clt_r_buf, Box::new(clt_r)),
                        Box::new(clt_w),
                        Box::new(ups_r),
                        Box::new(ups_w),
                    );
                    return StreamInspection::H1(h1_obj)
                        .into_loop_inspection(protocol_inspector)
                        .await;
                }
                _ => {
                    return Err(ServerTaskError::InvalidClientProtocol(
                        "unsupported client protocol",
                    ))
                }
            }
        }

        let copy_config = self.ctx.server_config.tcp_copy;
        let clt_to_ups =
            LimitedCopy::with_data(&mut clt_r, &mut ups_w, &copy_config, clt_r_buf.into());
        let ups_to_clt = LimitedCopy::new(&mut ups_r, &mut clt_w, &copy_config);
        crate::inspect::stream::transit_transparent2(
            clt_to_ups,
            ups_to_clt,
            &self.ctx.server_config,
            &self.ctx.server_quit_policy,
            None,
        )
        .await
    }
}
