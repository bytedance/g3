/*
 * SPDX-License-Identifier: Apache-2.0
 * Copyright 2023-2025 ByteDance and/or its affiliates.
 */

use std::net::{IpAddr, Ipv4Addr, Ipv6Addr, SocketAddr};
use std::sync::Arc;
use std::time::Duration;

use tokio::io::{AsyncRead, AsyncWrite};

use g3_daemon::server::ServerQuitPolicy;
use g3_daemon::stat::task::TcpStreamTaskStats;
use g3_io_ext::{IdleInterval, LimitedReader, LimitedWriter, StreamCopyConfig};
use g3_socks::{SocksVersion, v4a, v5};
use g3_types::acl::AclAction;
use g3_types::net::{ProxyRequestType, UpstreamAddr};

use super::{CommonTaskContext, TcpConnectTaskCltWrapperStats};
use crate::audit::AuditContext;
use crate::auth::User;
use crate::config::server::ServerConfig;
use crate::inspect::{StreamInspectContext, StreamTransitTask};
use crate::log::task::tcp_connect::TaskLogForTcpConnect;
use crate::module::tcp_connect::{TcpConnectTaskConf, TcpConnectTaskNotes};
use crate::serve::{
    ServerStats, ServerTaskError, ServerTaskForbiddenError, ServerTaskNotes, ServerTaskResult,
    ServerTaskStage,
};

pub(crate) struct SocksProxyTcpConnectTask {
    socks_version: SocksVersion,
    ctx: CommonTaskContext,
    upstream: UpstreamAddr,
    task_notes: ServerTaskNotes,
    tcp_notes: TcpConnectTaskNotes,
    task_stats: Arc<TcpStreamTaskStats>,
    audit_ctx: AuditContext,
    started: bool,
}

impl Drop for SocksProxyTcpConnectTask {
    fn drop(&mut self) {
        if self.started {
            self.post_stop();
            self.started = false;
        }
    }
}

impl SocksProxyTcpConnectTask {
    pub(crate) fn new(
        socks_version: SocksVersion,
        ctx: CommonTaskContext,
        mut task_notes: ServerTaskNotes,
        upstream: UpstreamAddr,
        audit_ctx: AuditContext,
    ) -> Self {
        if let Some(user_ctx) = task_notes.user_ctx_mut() {
            user_ctx.check_in_site(
                ctx.server_config.name(),
                ctx.server_stats.share_extra_tags(),
                &upstream,
            );
            if let Some(site_req_stats) = user_ctx.site_req_stats() {
                site_req_stats.conn_total.add_socks();
            }
        }
        SocksProxyTcpConnectTask {
            socks_version,
            ctx,
            upstream,
            task_notes,
            tcp_notes: TcpConnectTaskNotes::default(),
            task_stats: Arc::new(TcpStreamTaskStats::default()),
            audit_ctx,
            started: false,
        }
    }

    fn get_log_context(&self) -> Option<TaskLogForTcpConnect<'_>> {
        self.ctx
            .task_logger
            .as_ref()
            .map(|logger| TaskLogForTcpConnect {
                logger,
                upstream: &self.upstream,
                task_notes: &self.task_notes,
                tcp_notes: &self.tcp_notes,
                client_rd_bytes: self.task_stats.clt.read.get_bytes(),
                client_wr_bytes: self.task_stats.clt.write.get_bytes(),
                remote_rd_bytes: self.task_stats.ups.read.get_bytes(),
                remote_wr_bytes: self.task_stats.ups.write.get_bytes(),
            })
    }

    pub(crate) fn into_running<R, W>(mut self, clt_r: LimitedReader<R>, clt_w: LimitedWriter<W>)
    where
        R: AsyncRead + Send + Sync + Unpin + 'static,
        W: AsyncWrite + Send + Sync + Unpin + 'static,
    {
        tokio::spawn(async move {
            self.pre_start();
            let e = match self.run(clt_r, clt_w).await {
                Ok(_) => ServerTaskError::Finished,
                Err(e) => e,
            };
            if let Some(log_ctx) = self.get_log_context() {
                log_ctx.log(e);
            }
        });
    }

    fn pre_start(&mut self) {
        self.ctx.server_stats.task_tcp_connect.add_task();
        self.ctx.server_stats.task_tcp_connect.inc_alive_task();

        if let Some(user_ctx) = self.task_notes.user_ctx() {
            user_ctx.foreach_req_stats(|s| {
                s.req_total.add_socks_tcp_connect();
                s.req_alive.add_socks_tcp_connect();
            });
        }

        if self.ctx.server_config.flush_task_log_on_created {
            if let Some(log_ctx) = self.get_log_context() {
                log_ctx.log_created();
            }
        }

        self.started = true;
    }

    fn post_stop(&mut self) {
        self.ctx.server_stats.task_tcp_connect.dec_alive_task();

        if let Some(user_ctx) = self.task_notes.user_ctx() {
            user_ctx.foreach_req_stats(|s| s.req_alive.del_socks_tcp_connect());

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

            let action = user_ctx.check_upstream(&self.upstream);
            self.handle_user_acl_action(action, &mut clt_w, ServerTaskForbiddenError::DestDenied)
                .await?;

            tcp_client_misc_opts = user_ctx
                .user_config()
                .tcp_client_misc_opts(&tcp_client_misc_opts);
        }

        // server level dst host/port acl rules
        let action = self.ctx.check_upstream(&self.upstream);
        self.handle_server_upstream_acl_action(action, &mut clt_w)
            .await?;

        // set client side socket options
        self.ctx
            .cc_info
            .tcp_sock_set_raw_opts(&tcp_client_misc_opts, true)
            .map_err(|_| {
                ServerTaskError::InternalServerError("failed to set client socket options")
            })?;

        self.task_notes.stage = ServerTaskStage::Connecting;

        let task_conf = TcpConnectTaskConf {
            upstream: &self.upstream,
        };
        match self
            .ctx
            .escaper
            .tcp_setup_connection(
                &task_conf,
                &mut self.tcp_notes,
                &self.task_notes,
                self.task_stats.clone(),
                &mut self.audit_ctx,
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
        if self.ctx.server_config.flush_task_log_on_connected {
            if let Some(log_ctx) = self.get_log_context() {
                log_ctx.log_connected();
            }
        }

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
            user_ctx.foreach_req_stats(|s| s.req_ready.add_socks_tcp_connect());
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

        if let Some(audit_handle) = self.audit_ctx.handle() {
            let audit_task = self
                .task_notes
                .user_ctx()
                .map(|ctx| {
                    let user_config = &ctx.user_config().audit;
                    user_config.enable_protocol_inspection
                        && user_config
                            .do_task_audit()
                            .unwrap_or_else(|| audit_handle.do_task_audit())
                })
                .unwrap_or_else(|| audit_handle.do_task_audit());

            if audit_task {
                let ctx = StreamInspectContext::new(
                    audit_handle.clone(),
                    self.ctx.server_config.clone(),
                    self.ctx.server_stats.clone(),
                    self.ctx.server_quit_policy.clone(),
                    self.ctx.idle_wheel.clone(),
                    &self.task_notes,
                    &self.tcp_notes,
                );
                return crate::inspect::stream::transit_with_inspection(
                    clt_r,
                    clt_w,
                    ups_r,
                    ups_w,
                    ctx,
                    self.upstream.clone(),
                    None,
                )
                .await;
            }
        }

        self.transit_transparent(clt_r, clt_w, ups_r, ups_w).await
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
                self.ctx.server_stats.share_extra_tags(),
            ));

            let user_config = user_ctx.user_config();
            if !user_config
                .tcp_sock_speed_limit
                .eq(&self.ctx.server_config.tcp_sock_speed_limit)
            {
                let limit_config = user_config
                    .tcp_sock_speed_limit
                    .shrink_as_smaller(&self.ctx.server_config.tcp_sock_speed_limit);
                clt_r.reset_local_limit(limit_config.shift_millis, limit_config.max_north);
                clt_w.reset_local_limit(limit_config.shift_millis, limit_config.max_south);
            }

            let user = user_ctx.user();
            if let Some(limiter) = user.tcp_all_upload_speed_limit() {
                clt_r.add_global_limiter(limiter.clone());
            }
            if let Some(limiter) = user.tcp_all_download_speed_limit() {
                clt_w.add_global_limiter(limiter.clone());
            }
        }
        let wrapper_stats = Arc::new(wrapper_stats);
        clt_r.reset_stats(wrapper_stats.clone());
        clt_w.reset_stats(wrapper_stats);
    }
}

impl StreamTransitTask for SocksProxyTcpConnectTask {
    fn copy_config(&self) -> StreamCopyConfig {
        self.ctx.server_config.tcp_copy
    }

    fn idle_check_interval(&self) -> IdleInterval {
        self.ctx.idle_wheel.register()
    }

    fn max_idle_count(&self) -> usize {
        self.ctx.server_config.task_idle_max_count
    }

    fn log_client_shutdown(&self) {
        if let Some(log_ctx) = self.get_log_context() {
            log_ctx.log_client_shutdown();
        }
    }

    fn log_upstream_shutdown(&self) {
        if let Some(log_ctx) = self.get_log_context() {
            log_ctx.log_upstream_shutdown();
        }
    }

    fn log_periodic(&self) {
        if let Some(log_ctx) = self.get_log_context() {
            log_ctx.log_periodic();
        }
    }

    fn log_flush_interval(&self) -> Option<Duration> {
        self.ctx.log_flush_interval()
    }

    fn quit_policy(&self) -> &ServerQuitPolicy {
        self.ctx.server_quit_policy.as_ref()
    }

    fn user(&self) -> Option<&User> {
        self.task_notes.user_ctx().map(|ctx| ctx.user().as_ref())
    }
}
