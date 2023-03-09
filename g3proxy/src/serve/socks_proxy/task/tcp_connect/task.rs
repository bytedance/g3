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

use std::net::{IpAddr, Ipv4Addr, Ipv6Addr, SocketAddr};
use std::sync::Arc;

use log::debug;
use tokio::io::{AsyncRead, AsyncWrite};

use g3_io_ext::{LimitedReader, LimitedWriter};
use g3_socks::{v4a, v5, SocksVersion};
use g3_types::acl::AclAction;
use g3_types::net::{ProxyRequestType, UpstreamAddr};

use super::{CommonTaskContext, TcpConnectTaskCltWrapperStats, TcpConnectTaskStats};
use crate::config::server::ServerConfig;
use crate::inspect::StreamInspectContext;
use crate::log::task::tcp_connect::TaskLogForTcpConnect;
use crate::module::tcp_connect::TcpConnectTaskNotes;
use crate::serve::{
    ServerStats, ServerTaskError, ServerTaskForbiddenError, ServerTaskNotes, ServerTaskResult,
    ServerTaskStage,
};

pub(crate) struct SocksProxyTcpConnectTask {
    socks_version: SocksVersion,
    ctx: CommonTaskContext,
    task_notes: ServerTaskNotes,
    tcp_notes: TcpConnectTaskNotes,
    task_stats: Arc<TcpConnectTaskStats>,
}

impl SocksProxyTcpConnectTask {
    pub(crate) fn new(
        socks_version: SocksVersion,
        ctx: CommonTaskContext,
        mut task_notes: ServerTaskNotes,
        upstream: UpstreamAddr,
    ) -> Self {
        if let Some(user_ctx) = task_notes.user_ctx_mut() {
            user_ctx.check_in_site(
                ctx.server_config.name(),
                ctx.server_stats.extra_tags(),
                &upstream,
            );
        }
        SocksProxyTcpConnectTask {
            socks_version,
            ctx,
            task_notes,
            tcp_notes: TcpConnectTaskNotes::new(upstream),
            task_stats: Arc::new(TcpConnectTaskStats::new()),
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

    pub(crate) fn into_running<R, W>(mut self, clt_r: LimitedReader<R>, clt_w: LimitedWriter<W>)
    where
        R: AsyncRead + Send + Sync + Unpin + 'static,
        W: AsyncWrite + Send + Sync + Unpin + 'static,
    {
        tokio::spawn(async move {
            self.pre_start();
            match self.run(clt_r, clt_w).await {
                Ok(_) => self
                    .get_log_context()
                    .log(&self.ctx.task_logger, &ServerTaskError::Finished),
                Err(e) => self.get_log_context().log(&self.ctx.task_logger, &e),
            }
            self.pre_stop();
        });
    }

    fn pre_start(&self) {
        debug!(
            "Socks/TcpConnect: new client from {} to {} server {}, using escaper {}",
            self.ctx.tcp_client_addr,
            self.ctx.server_config.server_type(),
            self.ctx.server_config.name(),
            self.ctx.server_config.escaper
        );
        self.ctx.server_stats.task_tcp_connect.add_task();
        self.ctx.server_stats.task_tcp_connect.inc_alive_task();

        if let Some(user_ctx) = self.task_notes.user_ctx() {
            user_ctx.req_stats().req_total.add_socks_tcp_connect();
            user_ctx.req_stats().req_alive.add_socks_tcp_connect();

            if let Some(site_req_stats) = user_ctx.site_req_stats() {
                site_req_stats.req_total.add_socks_tcp_connect();
                site_req_stats.req_alive.add_socks_tcp_connect();
            }
        }
    }

    fn pre_stop(&mut self) {
        self.ctx.server_stats.task_tcp_connect.dec_alive_task();

        if let Some(user_ctx) = self.task_notes.user_ctx() {
            user_ctx.req_stats().req_alive.del_socks_tcp_connect();

            if let Some(site_req_stats) = user_ctx.site_req_stats() {
                site_req_stats.req_alive.del_socks_tcp_connect();
            }

            if let Some(user_req_alive_permit) = self.task_notes.user_req_alive_permit.take() {
                drop(user_req_alive_permit);
            }
        }
    }

    async fn reply_forbidden<W>(&self, clt_w: &mut W)
    where
        W: AsyncWrite + Unpin,
    {
        match self.socks_version {
            SocksVersion::V4a => {
                let _ = v4a::SocksV4Reply::RequestRejectedOrFailed.send(clt_w).await;
            }
            SocksVersion::V5 => {
                let _ = v5::Socks5Reply::ForbiddenByRule.send(clt_w).await;
            }
            SocksVersion::V6 => {} // TODO socks v6
        }
    }

    async fn handle_server_upstream_acl_action<W>(
        &self,
        action: AclAction,
        clt_w: &mut W,
    ) -> ServerTaskResult<()>
    where
        W: AsyncWrite + Unpin,
    {
        let forbid = match action {
            AclAction::Permit => false,
            AclAction::PermitAndLog => {
                // TODO log permit
                false
            }
            AclAction::Forbid => true,
            AclAction::ForbidAndLog => {
                // TODO log forbid
                true
            }
        };
        if forbid {
            self.ctx.server_stats.forbidden.add_dest_denied();
            if let Some(user_ctx) = self.task_notes.user_ctx() {
                // also add to user level forbidden stats
                user_ctx.add_dest_denied();
            }

            self.reply_forbidden(clt_w).await;
            Err(ServerTaskError::ForbiddenByRule(
                ServerTaskForbiddenError::DestDenied,
            ))
        } else {
            Ok(())
        }
    }

    async fn handle_user_acl_action<W>(
        &self,
        action: AclAction,
        clt_w: &mut W,
        forbidden_error: ServerTaskForbiddenError,
    ) -> ServerTaskResult<()>
    where
        W: AsyncWrite + Unpin,
    {
        let forbid = match action {
            AclAction::Permit => false,
            AclAction::PermitAndLog => {
                // TODO log permit
                false
            }
            AclAction::Forbid => true,
            AclAction::ForbidAndLog => {
                // TODO log forbid
                true
            }
        };
        if forbid {
            self.reply_forbidden(clt_w).await;
            Err(ServerTaskError::ForbiddenByRule(forbidden_error))
        } else {
            Ok(())
        }
    }

    async fn run<R, W>(
        &mut self,
        clt_r: LimitedReader<R>,
        mut clt_w: LimitedWriter<W>,
    ) -> ServerTaskResult<()>
    where
        R: AsyncRead + Send + Sync + Unpin + 'static,
        W: AsyncWrite + Send + Sync + Unpin + 'static,
    {
        let mut tcp_client_misc_opts = self.ctx.server_config.tcp_misc_opts;

        if let Some(user_ctx) = self.task_notes.user_ctx() {
            let user_ctx = user_ctx.clone();

            if user_ctx.check_rate_limit().is_err() {
                self.reply_forbidden(&mut clt_w).await;
                return Err(ServerTaskError::ForbiddenByRule(
                    ServerTaskForbiddenError::RateLimited,
                ));
            }

            match user_ctx.acquire_request_semaphore() {
                Ok(permit) => self.task_notes.user_req_alive_permit = Some(permit),
                Err(_) => {
                    self.reply_forbidden(&mut clt_w).await;
                    return Err(ServerTaskError::ForbiddenByRule(
                        ServerTaskForbiddenError::FullyLoaded,
                    ));
                }
            }

            let action = user_ctx.check_proxy_request(ProxyRequestType::SocksTcpConnect);
            self.handle_user_acl_action(action, &mut clt_w, ServerTaskForbiddenError::ProtoBanned)
                .await?;

            let action = user_ctx.check_upstream(&self.tcp_notes.upstream);
            self.handle_user_acl_action(action, &mut clt_w, ServerTaskForbiddenError::DestDenied)
                .await?;

            tcp_client_misc_opts = user_ctx
                .user()
                .config
                .tcp_client_misc_opts(&tcp_client_misc_opts);
        }

        // server level dst host/port acl rules
        let action = self.ctx.check_upstream(&self.tcp_notes.upstream);
        self.handle_server_upstream_acl_action(action, &mut clt_w)
            .await?;

        // set client side socket options
        g3_socket::tcp::set_raw_opts(self.ctx.tcp_client_socket, &tcp_client_misc_opts, true)
            .map_err(|_| {
                ServerTaskError::InternalServerError("failed to set client socket options")
            })?;

        self.task_notes.stage = ServerTaskStage::Connecting;
        match self
            .ctx
            .escaper
            .tcp_setup_connection(
                &mut self.tcp_notes,
                &self.task_notes,
                self.task_stats.for_escaper(),
            )
            .await
        {
            Ok((ups_r, ups_w)) => {
                self.task_notes.stage = ServerTaskStage::Connected;
                self.run_connected(clt_r, clt_w, ups_r, ups_w).await
            }
            Err(e) => {
                match self.socks_version {
                    SocksVersion::V4a => {
                        let _ = v4a::SocksV4Reply::RequestRejectedOrFailed
                            .send(&mut clt_w)
                            .await;
                    }
                    SocksVersion::V5 => {
                        let _ = v5::Socks5Reply::from(&e).send(&mut clt_w).await;
                    }
                    SocksVersion::V6 => {} // TODO socks v6
                }
                Err(e.into())
            }
        }
    }

    async fn run_connected<CR, CW, UR, UW>(
        &mut self,
        clt_r: LimitedReader<CR>,
        mut clt_w: LimitedWriter<CW>,
        ups_r: UR,
        ups_w: UW,
    ) -> ServerTaskResult<()>
    where
        CR: AsyncRead + Send + Sync + Unpin + 'static,
        CW: AsyncWrite + Send + Sync + Unpin + 'static,
        UR: AsyncRead + Send + Sync + Unpin + 'static,
        UW: AsyncWrite + Send + Sync + Unpin + 'static,
    {
        self.task_notes.stage = ServerTaskStage::Replying;
        match self.socks_version {
            SocksVersion::V4a => {
                v4a::SocksV4Reply::request_granted()
                    .send(&mut clt_w)
                    .await
                    .map_err(ServerTaskError::ClientTcpWriteFailed)?;
            }
            SocksVersion::V5 => {
                let addr = if let Some(addr) = &self.tcp_notes.chained.outgoing_addr {
                    *addr
                } else {
                    let (ip, port) = match &self.tcp_notes.local {
                        Some(addr) => (addr.ip(), addr.port()),
                        None => match self.tcp_notes.next {
                            Some(SocketAddr::V4(_)) => (IpAddr::V4(Ipv4Addr::UNSPECIFIED), 0),
                            Some(SocketAddr::V6(_)) => (IpAddr::V6(Ipv6Addr::UNSPECIFIED), 0),
                            None => unreachable!(),
                        },
                    };
                    SocketAddr::new(ip, port)
                };
                v5::Socks5Reply::Succeeded(addr)
                    .send(&mut clt_w)
                    .await
                    .map_err(ServerTaskError::ClientTcpWriteFailed)?;
            }
            SocksVersion::V6 => return Err(ServerTaskError::UnimplementedProtocol),
        }
        self.task_notes.mark_relaying();
        if let Some(user_ctx) = self.task_notes.user_ctx() {
            user_ctx.req_stats().req_ready.add_socks_tcp_connect();
            if let Some(site_req_stats) = user_ctx.site_req_stats() {
                site_req_stats.req_ready.add_socks_tcp_connect();
            }
        }
        self.relay(clt_r, clt_w, ups_r, ups_w).await
    }

    async fn relay<CR, CW, UR, UW>(
        &mut self,
        mut clt_r: LimitedReader<CR>,
        mut clt_w: LimitedWriter<CW>,
        ups_r: UR,
        ups_w: UW,
    ) -> ServerTaskResult<()>
    where
        CR: AsyncRead + Send + Sync + Unpin + 'static,
        CW: AsyncWrite + Send + Sync + Unpin + 'static,
        UR: AsyncRead + Send + Sync + Unpin + 'static,
        UW: AsyncWrite + Send + Sync + Unpin + 'static,
    {
        self.update_clt(&mut clt_r, &mut clt_w);

        if let Some(audit_handle) = &self.ctx.audit_handle {
            let do_protocol_inspection = self
                .task_notes
                .user_ctx()
                .map(|ctx| {
                    let user_config = &ctx.user().config.audit;
                    user_config.enable_protocol_inspection
                        && user_config
                            .do_application_audit()
                            .unwrap_or_else(|| audit_handle.do_application_audit())
                })
                .unwrap_or_else(|| audit_handle.do_application_audit());

            if do_protocol_inspection {
                let ctx = StreamInspectContext::new(
                    audit_handle.clone(),
                    self.ctx.server_config.clone(),
                    self.ctx.server_stats.clone(),
                    self.ctx.server_quit_policy.clone(),
                    &self.task_notes,
                );
                return crate::inspect::stream::transit_with_inspection(
                    clt_r,
                    clt_w,
                    ups_r,
                    ups_w,
                    ctx,
                    self.tcp_notes.upstream.clone(),
                    None,
                )
                .await;
            }
        }

        crate::inspect::stream::transit_transparent(
            clt_r,
            clt_w,
            ups_r,
            ups_w,
            &self.ctx.server_config,
            &self.ctx.server_quit_policy,
            self.task_notes.user_ctx().map(|ctx| ctx.user()),
        )
        .await
    }

    fn update_clt<CR, CW>(&mut self, clt_r: &mut LimitedReader<CR>, clt_w: &mut LimitedWriter<CW>)
    where
        CR: AsyncRead + Unpin,
        CW: AsyncWrite + Unpin,
    {
        let mut wrapper_stats =
            TcpConnectTaskCltWrapperStats::new(&self.ctx.server_stats, &self.task_stats);

        if let Some(user_ctx) = self.task_notes.user_ctx() {
            wrapper_stats.push_user_io_stats(user_ctx.fetch_traffic_stats(
                self.ctx.server_config.name(),
                self.ctx.server_stats.extra_tags(),
            ));

            let user = user_ctx.user();
            if !user
                .config
                .tcp_sock_speed_limit
                .eq(&self.ctx.server_config.tcp_sock_speed_limit)
            {
                let limit_config = user
                    .config
                    .tcp_sock_speed_limit
                    .shrink_as_smaller(&self.ctx.server_config.tcp_sock_speed_limit);
                clt_r.reset_limit(limit_config.shift_millis, limit_config.max_north);
                clt_w.reset_limit(limit_config.shift_millis, limit_config.max_south);
            }
        }
        let (clt_r_stats, clt_w_stats) = wrapper_stats.split();
        clt_r.reset_stats(clt_r_stats);
        clt_w.reset_stats(clt_w_stats);
    }
}
