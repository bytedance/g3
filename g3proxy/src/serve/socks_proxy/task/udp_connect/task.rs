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

use std::future::poll_fn;
use std::net::SocketAddr;
use std::sync::Arc;

use log::debug;
use slog::Logger;
use tokio::io::{AsyncRead, AsyncReadExt, AsyncWrite};
use tokio::net::UdpSocket;
use tokio::time::Instant;

use g3_io_ext::{
    LimitedUdpRecv, LimitedUdpSend, UdpCopyClientRecv, UdpCopyClientSend, UdpCopyClientToRemote,
    UdpCopyError, UdpCopyRemoteRecv, UdpCopyRemoteSend, UdpCopyRemoteToClient, UdpRecvHalf,
    UdpSendHalf,
};
use g3_socks::v5::Socks5Reply;
use g3_types::acl::AclAction;
use g3_types::net::{ProxyRequestType, UpstreamAddr};

use super::{
    CommonTaskContext, Socks5UdpConnectClientRecv, Socks5UdpConnectClientSend,
    UdpConnectTaskCltWrapperStats, UdpConnectTaskStats,
};
use crate::config::server::ServerConfig;
use crate::log::escape::udp_sendto::EscapeLogForUdpConnectSendTo;
use crate::log::task::udp_connect::TaskLogForUdpConnect;
use crate::module::udp_connect::UdpConnectTaskNotes;
use crate::serve::{
    ServerStats, ServerTaskError, ServerTaskForbiddenError, ServerTaskNotes, ServerTaskResult,
    ServerTaskStage,
};

pub(crate) struct SocksProxyUdpConnectTask {
    ctx: CommonTaskContext,
    udp_notes: UdpConnectTaskNotes,
    task_notes: ServerTaskNotes,
    task_stats: Arc<UdpConnectTaskStats>,
    udp_listen_addr: Option<SocketAddr>,
    udp_client_addr: Option<SocketAddr>,
}

impl SocksProxyUdpConnectTask {
    pub(crate) fn new(
        ctx: CommonTaskContext,
        notes: ServerTaskNotes,
        udp_client_addr: Option<SocketAddr>,
    ) -> Self {
        let buf_conf = ctx.server_config.udp_socket_buffer;
        SocksProxyUdpConnectTask {
            ctx,
            udp_notes: UdpConnectTaskNotes::empty(buf_conf),
            task_notes: notes,
            task_stats: Arc::new(UdpConnectTaskStats::default()),
            udp_listen_addr: None,
            udp_client_addr,
        }
    }

    fn get_log_context(&self) -> TaskLogForUdpConnect {
        TaskLogForUdpConnect {
            task_notes: &self.task_notes,
            tcp_server_addr: self.ctx.tcp_server_addr,
            tcp_client_addr: self.ctx.tcp_client_addr,
            udp_listen_addr: self.udp_listen_addr,
            udp_client_addr: self.udp_client_addr,
            udp_notes: &self.udp_notes,
            total_time: self.task_notes.time_elapsed(),
            client_rd_bytes: self.task_stats.clt.recv.get_bytes(),
            client_rd_packets: self.task_stats.clt.recv.get_packets(),
            client_wr_bytes: self.task_stats.clt.send.get_bytes(),
            client_wr_packets: self.task_stats.clt.send.get_packets(),
            remote_rd_bytes: self.task_stats.ups.recv.get_bytes(),
            remote_rd_packets: self.task_stats.ups.recv.get_packets(),
            remote_wr_bytes: self.task_stats.ups.send.get_bytes(),
            remote_wr_packets: self.task_stats.ups.send.get_packets(),
        }
    }

    pub(crate) fn into_running<R, W>(mut self, clt_r: R, clt_w: W)
    where
        R: AsyncRead + Send + Unpin + 'static,
        W: AsyncWrite + Send + Unpin + 'static,
    {
        tokio::spawn(async move {
            self.pre_start();
            match self.run(clt_r, clt_w).await {
                Ok(_) => self
                    .get_log_context()
                    .log(&self.ctx.task_logger, &ServerTaskError::ClosedByClient),
                Err(e) => self.get_log_context().log(&self.ctx.task_logger, &e),
            }
            self.pre_stop();
        });
    }

    fn pre_start(&self) {
        debug!(
            "SocksProxy/UdpConnect: new client from {} to {} server {}, using escaper {}",
            self.ctx.tcp_client_addr,
            self.ctx.server_config.server_type(),
            self.ctx.server_config.name(),
            self.ctx.server_config.escaper
        );
        self.ctx.server_stats.task_udp_connect.add_task();
        self.ctx.server_stats.task_udp_connect.inc_alive_task();

        if let Some(user_ctx) = self.task_notes.user_ctx() {
            user_ctx.req_stats().req_total.add_socks_udp_connect();
            user_ctx.req_stats().req_alive.add_socks_udp_connect();
        }
    }

    fn pre_stop(&mut self) {
        self.ctx.server_stats.task_udp_connect.dec_alive_task();

        if let Some(user_ctx) = self.task_notes.user_ctx() {
            user_ctx.req_stats().req_alive.del_socks_udp_connect();

            if let Some(site_req_stats) = user_ctx.site_req_stats() {
                site_req_stats.req_alive.del_socks_udp_connect();
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
        let _ = Socks5Reply::ForbiddenByRule.send(clt_w).await;
    }

    async fn handle_user_protocol_acl_action<W>(
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
            self.reply_forbidden(clt_w).await;
            Err(ServerTaskError::ForbiddenByRule(
                ServerTaskForbiddenError::ProtoBanned,
            ))
        } else {
            Ok(())
        }
    }

    fn handle_server_upstream_acl_action(&self, action: AclAction) -> ServerTaskResult<()> {
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

            Err(ServerTaskError::ForbiddenByRule(
                ServerTaskForbiddenError::DestDenied,
            ))
        } else {
            Ok(())
        }
    }

    fn handle_user_upstream_acl_action(&self, action: AclAction) -> ServerTaskResult<()> {
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
            Err(ServerTaskError::ForbiddenByRule(
                ServerTaskForbiddenError::DestDenied,
            ))
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
            self.handle_user_protocol_acl_action(action, &mut clt_tcp_w)
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
                Socks5Reply::Succeeded(SocketAddr::new(
                    self.ctx.get_mapped_udp_listen_ip(udp_listen_addr.ip()),
                    udp_listen_addr.port(),
                ))
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
            user_ctx.req_stats().req_ready.add_socks_udp_connect();
        }
        self.run_relay(
            clt_tcp_r,
            Box::new(clt_r),
            Box::new(clt_w),
            ups_r,
            ups_w,
            &escape_logger,
        )
        .await
    }

    async fn run_relay<'a, R>(
        &'a mut self,
        mut clt_tcp_r: R,
        clt_r: Box<dyn UdpCopyClientRecv + Unpin + Send>,
        clt_w: Box<dyn UdpCopyClientSend + Unpin + Send>,
        ups_r: Box<dyn UdpCopyRemoteRecv + Unpin + Send>,
        ups_w: Box<dyn UdpCopyRemoteSend + Unpin + Send>,
        escape_logger: &'a Logger,
    ) -> ServerTaskResult<()>
    where
        R: AsyncRead + Unpin,
    {
        let task_id = &self.task_notes.id;

        let mut c_to_r = UdpCopyClientToRemote::new(clt_r, ups_w, self.ctx.server_config.udp_relay);
        let mut r_to_c = UdpCopyRemoteToClient::new(clt_w, ups_r, self.ctx.server_config.udp_relay);

        let idle_duration = self.ctx.server_config.task_idle_check_duration;
        let mut idle_interval =
            tokio::time::interval_at(Instant::now() + idle_duration, idle_duration);
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
                        Err(UdpCopyError::RemoteError(e)) => {
                            EscapeLogForUdpConnectSendTo {
                                task_id,
                                udp_notes: &self.udp_notes,
                            }
                            .log(escape_logger, &e);
                            Err(e.into())
                        },
                        Err(UdpCopyError::ClientError(e)) => Err(e.into()),
                    };
                }
                r = &mut r_to_c => {
                    return match r {
                        Ok(_) => Ok(()),
                        Err(UdpCopyError::RemoteError(e)) => {
                            EscapeLogForUdpConnectSendTo {
                                task_id,
                                udp_notes: &self.udp_notes,
                            }
                            .log(escape_logger, &e);
                            Err(e.into())
                        },
                        Err(UdpCopyError::ClientError(e)) => Err(e.into()),
                    };
                }
                _ = idle_interval.tick() => {
                    if c_to_r.is_idle() && r_to_c.is_idle() {
                        idle_count += 1;

                        let quit = if let Some(user_ctx) = self.task_notes.user_ctx() {
                            let user = user_ctx.user();
                            if user.is_blocked() {
                                return Err(ServerTaskError::CanceledAsUserBlocked);
                            }
                            idle_count >= user.task_max_idle_count()
                        } else {
                            idle_count >= self.ctx.server_config.task_idle_max_count
                        };

                        if quit {
                            return Err(ServerTaskError::Idle(idle_duration, idle_count));
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
        Socks5UdpConnectClientRecv<LimitedUdpRecv<UdpRecvHalf>>,
        Socks5UdpConnectClientSend<LimitedUdpSend<UdpSendHalf>>,
        Box<dyn UdpCopyRemoteRecv + Unpin + Send>,
        Box<dyn UdpCopyRemoteSend + Unpin + Send>,
        Logger,
    )>
    where
        R: AsyncRead + Unpin,
    {
        let (clt_r, clt_w) = g3_io_ext::split_udp(clt_socket);

        let limit_config = if let Some(user_ctx) = self.task_notes.user_ctx() {
            user_ctx
                .user()
                .config
                .udp_sock_speed_limit
                .shrink_as_smaller(&self.ctx.server_config.udp_sock_speed_limit)
        } else {
            self.ctx.server_config.udp_sock_speed_limit
        };
        let (clt_r_stats, mut clt_w_stats) =
            UdpConnectTaskCltWrapperStats::new(&self.ctx.server_stats, &self.task_stats).split();

        let clt_r = LimitedUdpRecv::new(
            clt_r,
            limit_config.shift_millis,
            limit_config.max_north_packets,
            limit_config.max_north_bytes,
            clt_r_stats,
        );

        let mut clt_r = Socks5UdpConnectClientRecv::new(clt_r, self.udp_client_addr);

        let buf_len = self.ctx.server_config.udp_relay.packet_size();
        let mut buf = vec![0u8; buf_len].into_boxed_slice();

        let (buf_off, buf_nr, udp_client_addr, upstream) = self
            .recv_first_packet(clt_tcp_r, &mut clt_r, buf.as_mut())
            .await?;
        self.udp_notes.upstream = Some(upstream.clone());
        self.udp_client_addr = Some(udp_client_addr);

        if let Some(user_ctx) = self.task_notes.user_ctx_mut() {
            // set user site by using the upstream address of the first packet
            user_ctx.check_in_site(
                self.ctx.server_config.name(),
                self.ctx.server_stats.extra_tags(),
                &upstream,
            );

            if let Some(site_req_stats) = user_ctx.site_req_stats() {
                site_req_stats.conn_total.add_socks();
                site_req_stats.req_total.add_socks_udp_connect();
                site_req_stats.req_alive.add_socks_udp_connect();
            }

            let mut wrapper_stats =
                UdpConnectTaskCltWrapperStats::new(&self.ctx.server_stats, &self.task_stats);
            let user_io_stats = user_ctx.fetch_traffic_stats(
                self.ctx.server_config.name(),
                self.ctx.server_stats.extra_tags(),
            );

            let p1_size = buf_nr - buf_off;
            for s in &user_io_stats {
                s.io.socks_udp_connect.add_in_bytes(p1_size as u64);
                s.io.socks_udp_connect.add_in_packet();
            }

            wrapper_stats.push_user_io_stats(user_io_stats);
            let (clt_r_stats, new_clt_w_stats) = wrapper_stats.split();
            clt_r.inner_mut().reset_stats(clt_r_stats);
            clt_w_stats = new_clt_w_stats;
        }

        if let Some(user_ctx) = self.task_notes.user_ctx() {
            let action = user_ctx.check_upstream(&upstream);
            self.handle_user_upstream_acl_action(action)?;
        }
        let action = self.ctx.check_upstream(&upstream);
        self.handle_server_upstream_acl_action(action)?;

        clt_r
            .inner()
            .inner()
            .connect(udp_client_addr)
            .await
            .map_err(|_| {
                ServerTaskError::InternalServerError("unable to connect the client side udp socket")
            })?;
        let clt_w = LimitedUdpSend::new(
            clt_w,
            limit_config.shift_millis,
            limit_config.max_south_packets,
            limit_config.max_south_bytes,
            clt_w_stats,
        );

        self.task_notes.stage = ServerTaskStage::Connecting;
        let (ups_r, mut ups_w, logger) = self
            .ctx
            .escaper
            .udp_setup_connection(
                &mut self.udp_notes,
                &self.task_notes,
                self.task_stats.for_escaper(),
            )
            .await?;
        self.task_notes.stage = ServerTaskStage::Connected;

        poll_fn(|cx| ups_w.poll_send_packet(cx, buf.as_mut(), buf_off, buf_nr)).await?;

        let clt_w = Socks5UdpConnectClientSend::new(clt_w, upstream);

        Ok((clt_r, clt_w, ups_r, ups_w, logger))
    }

    async fn recv_first_packet<R>(
        &self,
        clt_tcp_r: &mut R,
        clt_udp_r: &mut Socks5UdpConnectClientRecv<LimitedUdpRecv<UdpRecvHalf>>,
        buf: &mut [u8],
    ) -> ServerTaskResult<(usize, usize, SocketAddr, UpstreamAddr)>
    where
        R: AsyncRead + Unpin,
    {
        let udp_fut = tokio::time::timeout(
            self.ctx.server_config.timeout.udp_client_initial,
            clt_udp_r.recv_first_packet(buf, &self.ctx.ingress_net_filter),
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
                    Ok(Ok((buf_off, buf_nr, udp_client_addr, upstream))) => {
                        Ok((buf_off, buf_nr, udp_client_addr, upstream))
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
