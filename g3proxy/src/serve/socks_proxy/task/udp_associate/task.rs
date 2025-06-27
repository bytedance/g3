/*
 * SPDX-License-Identifier: Apache-2.0
 * Copyright 2023-2025 ByteDance and/or its affiliates.
 */

use std::future::poll_fn;
use std::net::SocketAddr;
use std::sync::Arc;

use slog::Logger;
use tokio::io::{AsyncRead, AsyncReadExt, AsyncWrite};
use tokio::net::UdpSocket;

use g3_io_ext::{
    LimitedUdpRecv, LimitedUdpSend, UdpRecvHalf, UdpRelayClientRecv, UdpRelayClientSend,
    UdpRelayClientToRemote, UdpRelayError, UdpRelayRemoteRecv, UdpRelayRemoteSend,
    UdpRelayRemoteToClient, UdpSendHalf,
};
use g3_socks::v5::Socks5Reply;
use g3_types::acl::AclAction;
use g3_types::net::{ProxyRequestType, UpstreamAddr};

use super::{
    CommonTaskContext, Socks5UdpAssociateClientRecv, Socks5UdpAssociateClientSend,
    UdpAssociateTaskCltWrapperStats, UdpAssociateTaskStats,
};
use crate::config::server::ServerConfig;
use crate::log::escape::udp_sendto::EscapeLogForUdpRelaySendto;
use crate::log::task::udp_associate::TaskLogForUdpAssociate;
use crate::module::udp_relay::{UdpRelayTaskConf, UdpRelayTaskNotes};
use crate::serve::{
    ServerStats, ServerTaskError, ServerTaskForbiddenError, ServerTaskNotes, ServerTaskResult,
    ServerTaskStage,
};

pub(crate) struct SocksProxyUdpAssociateTask {
    ctx: Arc<CommonTaskContext>,
    initial_peer: UpstreamAddr,
    udp_notes: UdpRelayTaskNotes,
    task_notes: ServerTaskNotes,
    task_stats: Arc<UdpAssociateTaskStats>,
    udp_listen_addr: Option<SocketAddr>,
    udp_client_addr: Option<SocketAddr>,
    max_idle_count: usize,
    started: bool,
}

impl Drop for SocksProxyUdpAssociateTask {
    fn drop(&mut self) {
        if self.started {
            self.post_stop();
            self.started = false;
        }
    }
}

impl SocksProxyUdpAssociateTask {
    pub(crate) fn new(
        ctx: CommonTaskContext,
        notes: ServerTaskNotes,
        udp_client_addr: Option<SocketAddr>,
    ) -> Self {
        let max_idle_count = notes
            .user_ctx()
            .and_then(|c| c.user().task_max_idle_count())
            .unwrap_or(ctx.server_config.task_idle_max_count);
        SocksProxyUdpAssociateTask {
            ctx: Arc::new(ctx),
            initial_peer: UpstreamAddr::empty(),
            udp_notes: UdpRelayTaskNotes::default(),
            task_notes: notes,
            task_stats: Arc::new(UdpAssociateTaskStats::default()),
            udp_listen_addr: None,
            udp_client_addr,
            max_idle_count,
            started: false,
        }
    }

    fn get_log_context(&self) -> Option<TaskLogForUdpAssociate<'_>> {
        self.ctx
            .task_logger
            .as_ref()
            .map(|logger| TaskLogForUdpAssociate {
                logger,
                task_notes: &self.task_notes,
                tcp_server_addr: self.ctx.server_addr(),
                tcp_client_addr: self.ctx.client_addr(),
                udp_listen_addr: self.udp_listen_addr,
                udp_client_addr: self.udp_client_addr,
                initial_peer: &self.initial_peer,
                udp_notes: &self.udp_notes,
                client_rd_bytes: self.task_stats.clt.recv.get_bytes(),
                client_rd_packets: self.task_stats.clt.recv.get_packets(),
                client_wr_bytes: self.task_stats.clt.send.get_bytes(),
                client_wr_packets: self.task_stats.clt.send.get_packets(),
                remote_rd_bytes: self.task_stats.ups.recv.get_bytes(),
                remote_rd_packets: self.task_stats.ups.recv.get_packets(),
                remote_wr_bytes: self.task_stats.ups.send.get_bytes(),
                remote_wr_packets: self.task_stats.ups.send.get_packets(),
            })
    }

    pub(crate) fn into_running<R, W>(mut self, clt_r: R, clt_w: W)
    where
        R: AsyncRead + Send + Unpin + 'static,
        W: AsyncWrite + Send + Unpin + 'static,
    {
        tokio::spawn(async move {
            self.pre_start();
            let e = match self.run(clt_r, clt_w).await {
                Ok(_) => ServerTaskError::ClosedByClient,
                Err(e) => e,
            };
            if let Some(log_ctx) = self.get_log_context() {
                log_ctx.log(e);
            }
        });
    }

    fn pre_start(&mut self) {
        self.ctx.server_stats.task_udp_associate.add_task();
        self.ctx.server_stats.task_udp_associate.inc_alive_task();

        if let Some(user_ctx) = self.task_notes.user_ctx() {
            user_ctx.req_stats().req_total.add_socks_udp_associate();
            user_ctx.req_stats().req_alive.add_socks_udp_associate();
        }

        if self.ctx.server_config.flush_task_log_on_created {
            if let Some(log_ctx) = self.get_log_context() {
                log_ctx.log_created();
            }
        }

        self.started = true;
    }

    fn post_stop(&mut self) {
        self.ctx.server_stats.task_udp_associate.dec_alive_task();

        if let Some(user_ctx) = self.task_notes.user_ctx() {
            user_ctx.foreach_req_stats(|s| s.req_alive.del_socks_udp_associate());

            if let Some(user_req_alive_permit) = self.task_notes.user_req_alive_permit.take() {
                drop(user_req_alive_permit);
            }
        }
    }

    async fn reply_forbidden<W>(&self, clt_w: &mut W)
    where
        W: AsyncWrite + Unpin,
    {
        let _ = Socks5Reply::ForbiddenByRule.send(clt_w).await;
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

    pub(crate) async fn run<R, W>(
        &mut self,
        mut clt_tcp_r: R,
        mut clt_tcp_w: W,
    ) -> ServerTaskResult<()>
    where
        R: AsyncRead + Unpin,
        W: AsyncWrite + Unpin,
    {
        if let Some(user_ctx) = self.task_notes.user_ctx() {
            let user_ctx = user_ctx.clone();

            if user_ctx.check_rate_limit().is_err() {
                self.reply_forbidden(&mut clt_tcp_w).await;
                return Err(ServerTaskError::ForbiddenByRule(
                    ServerTaskForbiddenError::RateLimited,
                ));
            }

            match user_ctx.acquire_request_semaphore() {
                Ok(permit) => self.task_notes.user_req_alive_permit = Some(permit),
                Err(_) => {
                    self.reply_forbidden(&mut clt_tcp_w).await;
                    return Err(ServerTaskError::ForbiddenByRule(
                        ServerTaskForbiddenError::FullyLoaded,
                    ));
                }
            }

            let action = user_ctx.check_proxy_request(ProxyRequestType::SocksUdpAssociate);
            self.handle_user_acl_action(
                action,
                &mut clt_tcp_w,
                ServerTaskForbiddenError::ProtoBanned,
            )
            .await?;
        }

        self.task_notes.stage = ServerTaskStage::Preparing;
        let clt_socket = match self
            .ctx
            .setup_udp_listen(self.udp_client_addr, &self.task_notes)
            .await
        {
            Ok((udp_listen_addr, socket)) => {
                self.task_notes.stage = ServerTaskStage::Replying;
                self.udp_listen_addr = Some(udp_listen_addr);
                let udp_echo_addr = self
                    .ctx
                    .server_config
                    .transmute_udp_echo_addr(udp_listen_addr);
                Socks5Reply::Succeeded(udp_echo_addr)
                    .send(&mut clt_tcp_w)
                    .await
                    .map_err(ServerTaskError::ClientTcpWriteFailed)?;
                socket
            }
            Err(e) => {
                let _ = Socks5Reply::GeneralServerFailure.send(&mut clt_tcp_w).await;
                return Err(e);
            }
        };

        let (clt_r, clt_w, ups_r, ups_w, escape_logger) =
            self.split_all(&mut clt_tcp_r, clt_socket).await?;

        self.task_notes.mark_relaying();
        if let Some(user_ctx) = self.task_notes.user_ctx() {
            user_ctx.foreach_req_stats(|s| s.req_ready.add_socks_udp_associate());
        }
        self.run_relay(
            clt_tcp_r,
            Box::new(clt_r),
            Box::new(clt_w),
            ups_r,
            ups_w,
            escape_logger,
        )
        .await
    }

    async fn run_relay<R>(
        &mut self,
        mut clt_tcp_r: R,
        mut clt_r: Box<dyn UdpRelayClientRecv + Unpin + Send>,
        mut clt_w: Box<dyn UdpRelayClientSend + Unpin + Send>,
        mut ups_r: Box<dyn UdpRelayRemoteRecv + Unpin + Send>,
        mut ups_w: Box<dyn UdpRelayRemoteSend + Unpin + Send>,
        escape_logger: Option<Logger>,
    ) -> ServerTaskResult<()>
    where
        R: AsyncRead + Unpin,
    {
        let task_id = &self.task_notes.id;

        let mut c_to_r =
            UdpRelayClientToRemote::new(&mut *clt_r, &mut *ups_w, self.ctx.server_config.udp_relay);
        let mut r_to_c =
            UdpRelayRemoteToClient::new(&mut *clt_w, &mut *ups_r, self.ctx.server_config.udp_relay);

        let mut idle_interval = self.ctx.idle_wheel.register();
        let mut log_interval = self.ctx.get_log_interval();
        let mut idle_count = 0;
        let mut buf: [u8; 4] = [0; 4];
        loop {
            tokio::select! {
                biased;

                r = clt_tcp_r.read(&mut buf) => {
                    return match r {
                        Ok(0) => Ok(()),
                        Ok(_) => {
                            Err(ServerTaskError::InvalidClientProtocol(
                                "unexpected data received from the tcp channel"
                            ))
                        }
                        Err(e) => Err(ServerTaskError::ClientTcpReadFailed(e)),
                    };
                }
                r = &mut c_to_r => {
                    return match r {
                        Ok(_) => Ok(()),
                        Err(UdpRelayError::RemoteError(ra, e)) => {
                            if let Some(logger) = escape_logger {
                                EscapeLogForUdpRelaySendto {
                                    task_id,
                                    udp_notes: &self.udp_notes,
                                    remote_addr: &ra,
                                }
                                .log(&logger, &e);
                            }
                            Err(e.into())
                        }
                        Err(UdpRelayError::ClientError(e)) => Err(e.into()),
                    };
                }
                r = &mut r_to_c => {
                    return match r {
                        Ok(_) => Ok(()),
                        Err(UdpRelayError::RemoteError(ra, e)) => {
                            if let Some(logger) = escape_logger {
                                EscapeLogForUdpRelaySendto {
                                    task_id,
                                    udp_notes: &self.udp_notes,
                                    remote_addr: &ra,
                                }
                                .log(&logger, &e);
                            }
                            return Err(e.into());
                        }
                        Err(UdpRelayError::ClientError(e)) => Err(e.into()),
                    };
                }
                 _ = log_interval.tick() => {
                    if let Some(log_ctx) = self.get_log_context() {
                        log_ctx.log_periodic();
                    }
                }
                n = idle_interval.tick() => {
                    if c_to_r.is_idle() && r_to_c.is_idle() {
                        idle_count += n;

                        if let Some(user_ctx) = self.task_notes.user_ctx() {
                            let user = user_ctx.user();
                            if user.is_blocked() {
                                return Err(ServerTaskError::CanceledAsUserBlocked);
                            }
                        }

                        if idle_count >= self.max_idle_count {
                            return Err(ServerTaskError::Idle(idle_interval.period(), idle_count));
                        }
                    } else {
                        idle_count = 0;

                        c_to_r.reset_active();
                        r_to_c.reset_active();
                    }

                    if let Some(user_ctx) = self.task_notes.user_ctx() {
                        if user_ctx.user().is_blocked() {
                            return Err(ServerTaskError::CanceledAsUserBlocked);
                        }
                    }

                    if self.ctx.server_quit_policy.force_quit() {
                        return Err(ServerTaskError::CanceledAsServerQuit)
                    }
                }
            }
        }
    }

    async fn split_all<R>(
        &mut self,
        clt_tcp_r: &mut R,
        clt_socket: UdpSocket,
    ) -> ServerTaskResult<(
        Socks5UdpAssociateClientRecv<LimitedUdpRecv<UdpRecvHalf>>,
        Socks5UdpAssociateClientSend<LimitedUdpSend<UdpSendHalf>>,
        Box<dyn UdpRelayRemoteRecv + Unpin + Send>,
        Box<dyn UdpRelayRemoteSend + Unpin + Send>,
        Option<Logger>,
    )>
    where
        R: AsyncRead + Unpin,
    {
        let (clt_r, clt_w) = g3_io_ext::split_udp(clt_socket);

        let limit_config = if let Some(user_ctx) = self.task_notes.user_ctx() {
            user_ctx
                .user_config()
                .udp_sock_speed_limit
                .shrink_as_smaller(&self.ctx.server_config.udp_sock_speed_limit)
        } else {
            self.ctx.server_config.udp_sock_speed_limit
        };
        let wrapper_stats = Arc::new(UdpAssociateTaskCltWrapperStats::new(
            &self.ctx.server_stats,
            &self.task_stats,
        ));

        let mut clt_r = LimitedUdpRecv::local_limited(
            clt_r,
            limit_config.shift_millis,
            limit_config.max_north_packets,
            limit_config.max_north_bytes,
            wrapper_stats.clone(),
        );
        if let Some(user_ctx) = self.task_notes.user_ctx() {
            if let Some(limiter) = user_ctx.user().udp_all_upload_speed_limit() {
                clt_r.add_global_limiter(limiter.clone());
            }
        }
        let mut clt_w_stats = wrapper_stats;

        let mut clt_r = Socks5UdpAssociateClientRecv::new(
            clt_r,
            self.udp_client_addr,
            &self.ctx,
            self.task_notes.user_ctx(),
        );

        let buf_len = self.ctx.server_config.udp_relay.packet_size();
        let mut buf = vec![0u8; buf_len];

        let (buf_off, buf_nr, udp_client_addr) = self
            .recv_first_packet(clt_tcp_r, &mut clt_r, &mut buf)
            .await?;
        self.udp_client_addr = Some(udp_client_addr);

        if let Some(user_ctx) = self.task_notes.user_ctx_mut() {
            // set user site by using the upstream address of the first packet
            user_ctx.check_in_site(
                self.ctx.server_config.name(),
                self.ctx.server_stats.share_extra_tags(),
                &self.initial_peer,
            );

            if let Some(site_req_stats) = user_ctx.site_req_stats() {
                site_req_stats.conn_total.add_socks();
                site_req_stats.req_total.add_socks_udp_associate();
                site_req_stats.req_alive.add_socks_udp_associate();
            }

            let mut wrapper_stats =
                UdpAssociateTaskCltWrapperStats::new(&self.ctx.server_stats, &self.task_stats);
            let user_io_stats = user_ctx.fetch_traffic_stats(
                self.ctx.server_config.name(),
                self.ctx.server_stats.share_extra_tags(),
            );

            let p1_size = buf_nr - buf_off;
            for s in &user_io_stats {
                s.io.socks_udp_associate.add_in_bytes(p1_size as u64);
                s.io.socks_udp_associate.add_in_packet();
            }

            wrapper_stats.push_user_io_stats(user_io_stats);
            let wrapper_stats = Arc::new(wrapper_stats);
            clt_r.inner_mut().reset_stats(wrapper_stats.clone());
            clt_w_stats = wrapper_stats;
        }

        clt_r
            .inner()
            .inner()
            .connect(udp_client_addr)
            .await
            .map_err(|_| {
                ServerTaskError::InternalServerError("unable to connect the client side udp socket")
            })?;

        let mut clt_w = LimitedUdpSend::local_limited(
            clt_w,
            limit_config.shift_millis,
            limit_config.max_south_packets,
            limit_config.max_south_bytes,
            clt_w_stats,
        );
        if let Some(user_ctx) = self.task_notes.user_ctx() {
            if let Some(limiter) = user_ctx.user().udp_all_download_speed_limit() {
                clt_w.add_global_limiter(limiter.clone());
            }
        }

        self.task_notes.stage = ServerTaskStage::Connecting;

        let task_conf = UdpRelayTaskConf {
            initial_peer: &self.initial_peer,
            sock_buf: self.ctx.server_config.udp_socket_buffer,
        };
        let (ups_r, mut ups_w, logger) = self
            .ctx
            .escaper
            .udp_setup_relay(
                &task_conf,
                &mut self.udp_notes,
                &self.task_notes,
                self.task_stats.clone(),
            )
            .await?;
        self.task_notes.stage = ServerTaskStage::Connected;

        if self.ctx.server_config.flush_task_log_on_connected {
            if let Some(log_ctx) = self.get_log_context() {
                log_ctx.log_connected();
            }
        }

        poll_fn(|cx| ups_w.poll_send_packet(cx, &buf[buf_off..buf_nr], &self.initial_peer)).await?;

        let clt_w = Socks5UdpAssociateClientSend::new(clt_w, udp_client_addr);

        Ok((clt_r, clt_w, ups_r, ups_w, logger))
    }

    async fn recv_first_packet<R>(
        &mut self,
        clt_tcp_r: &mut R,
        clt_udp_r: &mut Socks5UdpAssociateClientRecv<LimitedUdpRecv<UdpRecvHalf>>,
        buf: &mut [u8],
    ) -> ServerTaskResult<(usize, usize, SocketAddr)>
    where
        R: AsyncRead + Unpin,
    {
        let udp_fut = tokio::time::timeout(
            self.ctx.server_config.timeout.udp_client_initial,
            clt_udp_r.recv_first_packet(buf, &self.ctx.ingress_net_filter, &mut self.initial_peer),
        );
        let mut buf_tcp: [u8; 4] = [0; 4];
        tokio::select! {
            biased;

            ret = clt_tcp_r.read(&mut buf_tcp) => {
                match ret {
                    Ok(0) => Err(ServerTaskError::ClosedByClient),
                    Ok(_) => {
                        Err(ServerTaskError::InvalidClientProtocol(
                            "unexpected data received from the tcp channel"
                        ))
                    }
                    Err(e) => Err(ServerTaskError::ClientTcpReadFailed(e)),
                }
            }
            ret = udp_fut => {
                match ret {
                    Ok(Ok((buf_off, buf_nr, udp_client_addr))) => {
                        Ok((buf_off, buf_nr, udp_client_addr))
                    }
                    Ok(Err(e)) => Err(e.into()),
                    Err(_) => {
                        Err(ServerTaskError::ClientAppTimeout(
                            "timeout to wait first udp packet"
                        ))
                    }
                }
            }
        }
    }
}
