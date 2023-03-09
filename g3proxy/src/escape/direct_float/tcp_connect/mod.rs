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

use std::io;
use std::net::{IpAddr, SocketAddr};

use tokio::net::{TcpSocket, TcpStream};
use tokio::task::JoinSet;
use tokio::time::Instant;

use g3_daemon::stat::remote::ArcTcpConnectionTaskRemoteStats;
use g3_io_ext::{LimitedReader, LimitedWriter};
use g3_socket::util::AddressFamily;
use g3_types::acl::AclAction;
use g3_types::net::{ConnectError, Host, TcpConnectConfig, TcpKeepAliveConfig, TcpMiscSockOpts};

use super::{DirectFloatBindIp, DirectFloatEscaper, DirectFloatEscaperStats};
use crate::log::escape::tcp_connect::EscapeLogForTcpConnect;
use crate::module::tcp_connect::{TcpConnectError, TcpConnectResult, TcpConnectTaskNotes};
use crate::resolve::HappyEyeballsResolveJob;
use crate::serve::ServerTaskNotes;

mod stats;
use stats::DirectTcpMixedRemoteStats;

impl DirectFloatEscaper {
    fn handle_tcp_target_ip_acl_action<'a>(
        &'a self,
        action: AclAction,
        task_notes: &'a ServerTaskNotes,
    ) -> Result<(), TcpConnectError> {
        let forbid = match action {
            AclAction::Permit => false,
            AclAction::PermitAndLog => {
                // TODO log
                false
            }
            AclAction::Forbid => true,
            AclAction::ForbidAndLog => {
                // TODO log
                true
            }
        };
        if forbid {
            self.stats.forbidden.add_ip_blocked();
            if let Some(user_ctx) = task_notes.user_ctx() {
                user_ctx.add_ip_blocked();
            }
            Err(TcpConnectError::ForbiddenRemoteAddress)
        } else {
            Ok(())
        }
    }

    fn prepare_connect_socket(
        &self,
        peer_ip: IpAddr,
        bind_ip: Option<IpAddr>,
        task_notes: &ServerTaskNotes,
        keepalive: &TcpKeepAliveConfig,
        misc_opts: &TcpMiscSockOpts,
    ) -> Result<(TcpSocket, DirectFloatBindIp), TcpConnectError> {
        match peer_ip {
            IpAddr::V4(_) => {
                if self.config.no_ipv4 {
                    return Err(TcpConnectError::ForbiddenAddressFamily);
                }
            }
            IpAddr::V6(_) => {
                if self.config.no_ipv6 {
                    return Err(TcpConnectError::ForbiddenAddressFamily);
                }
            }
        }

        let (_, action) = self.egress_net_filter.check(peer_ip);
        self.handle_tcp_target_ip_acl_action(action, task_notes)?;

        let bind = if let Some(ip) = bind_ip {
            self.get_bind_again(ip).ok_or_else(|| {
                TcpConnectError::SetupSocketFailed(io::Error::new(
                    io::ErrorKind::AddrNotAvailable,
                    "bind ip expired",
                ))
            })?
        } else {
            self.get_bind_random(AddressFamily::from(&peer_ip))
                .ok_or_else(|| {
                    TcpConnectError::SetupSocketFailed(io::Error::new(
                        io::ErrorKind::AddrNotAvailable,
                        "no bind ip usable",
                    ))
                })?
        };

        let sock =
            g3_socket::tcp::new_socket_to(peer_ip, Some(bind.ip), keepalive, misc_opts, true)
                .map_err(TcpConnectError::SetupSocketFailed)?;
        Ok((sock, bind))
    }

    async fn fixed_try_connect(
        &self,
        peer_ip: IpAddr,
        tcp_connect_config: TcpConnectConfig,
        keepalive: TcpKeepAliveConfig,
        tcp_misc_opts: TcpMiscSockOpts,
        tcp_notes: &mut TcpConnectTaskNotes,
        task_notes: &ServerTaskNotes,
    ) -> Result<(TcpStream, DirectFloatBindIp), TcpConnectError> {
        let (sock, bind) = self.prepare_connect_socket(
            peer_ip,
            tcp_notes.bind,
            task_notes,
            &keepalive,
            &tcp_misc_opts,
        )?;
        let peer = SocketAddr::new(peer_ip, tcp_notes.upstream.port());
        tcp_notes.next = Some(peer);
        tcp_notes.bind = Some(bind.ip);
        tcp_notes.expire = bind.expire_datetime;
        tcp_notes.egress = Some(bind.egress_info.clone());

        let instant_now = Instant::now();

        self.stats.tcp.add_connection_attempted();
        tcp_notes.tries = 1;
        match tokio::time::timeout(tcp_connect_config.each_timeout(), sock.connect(peer)).await {
            Ok(Ok(ups_stream)) => {
                tcp_notes.duration = instant_now.elapsed();

                self.stats.tcp.add_connection_established();
                let local_addr = ups_stream
                    .local_addr()
                    .map_err(TcpConnectError::SetupSocketFailed)?;
                tcp_notes.local = Some(local_addr);
                tcp_notes.chained.target_addr = Some(peer);
                tcp_notes.chained.outgoing_addr = Some(local_addr);
                Ok((ups_stream, bind))
            }
            Ok(Err(e)) => {
                tcp_notes.duration = instant_now.elapsed();

                let e = TcpConnectError::ConnectFailed(ConnectError::from(e));
                EscapeLogForTcpConnect {
                    tcp_notes,
                    task_id: &task_notes.id,
                }
                .log(&self.escape_logger, &e);
                Err(e)
            }
            Err(_) => {
                tcp_notes.duration = instant_now.elapsed();

                let e = TcpConnectError::TimeoutByRule;
                EscapeLogForTcpConnect {
                    tcp_notes,
                    task_id: &task_notes.id,
                }
                .log(&self.escape_logger, &e);
                Err(e)
            }
        }
    }

    fn merge_ip_list(&self, tried: usize, ips: &mut Vec<IpAddr>, new: Vec<IpAddr>) {
        self.config.happy_eyeballs.merge_list(tried, ips, new);
    }

    async fn happy_try_connect(
        &self,
        mut resolver_job: HappyEyeballsResolveJob,
        tcp_connect_config: TcpConnectConfig,
        keepalive: TcpKeepAliveConfig,
        tcp_misc_opts: TcpMiscSockOpts,
        tcp_notes: &mut TcpConnectTaskNotes,
        task_notes: &ServerTaskNotes,
    ) -> Result<(TcpStream, DirectFloatBindIp), TcpConnectError> {
        let max_tries_each_family = tcp_connect_config.max_tries();
        let mut ips = resolver_job
            .get_r1_or_first(
                self.config.happy_eyeballs.resolution_delay(),
                max_tries_each_family,
            )
            .await?;
        let port = tcp_notes.upstream.port();

        let mut c_set = JoinSet::new();

        let mut connect_interval =
            tokio::time::interval(self.config.happy_eyeballs.connection_attempt_delay());
        // connect_interval.tick().await; will take 1ms
        // let's use local vars to skip the first tick()
        let mut skip_first_tick = true;

        let mut spawn_new_connection = true;
        let mut running_connection = 0;
        let mut resolver_r2_done = false;
        let each_timeout = tcp_connect_config.each_timeout();

        tcp_notes.tries = 0;
        let instant_now = Instant::now();
        let mut returned_err = TcpConnectError::NoAddressConnected;

        loop {
            if spawn_new_connection {
                if let Some(ip) = ips.pop() {
                    let (sock, bind) = self.prepare_connect_socket(
                        ip,
                        tcp_notes.bind,
                        task_notes,
                        &keepalive,
                        &tcp_misc_opts,
                    )?;
                    let peer = SocketAddr::new(ip, port);
                    running_connection += 1;
                    spawn_new_connection = false;
                    tcp_notes.tries += 1;
                    self.stats.tcp.add_connection_attempted();
                    c_set.spawn(async move {
                        match tokio::time::timeout(each_timeout, sock.connect(peer)).await {
                            Ok(Ok(stream)) => (Ok(stream), peer, bind),
                            Ok(Err(e)) => (
                                Err(TcpConnectError::ConnectFailed(ConnectError::from(e))),
                                peer,
                                bind,
                            ),
                            Err(_) => (Err(TcpConnectError::TimeoutByRule), peer, bind),
                        }
                    });
                    connect_interval.reset();
                }
            }

            if running_connection > 0 {
                tokio::select! {
                    biased;

                    r = c_set.join_next() => {
                        tcp_notes.duration = instant_now.elapsed();
                        match r {
                            Some(Ok(r)) => {
                                running_connection -= 1;
                                let peer_addr = r.1;
                                let bind = r.2;
                                tcp_notes.next = Some(peer_addr);
                                tcp_notes.bind = Some(bind.ip);
                                tcp_notes.expire = bind.expire_datetime;
                                tcp_notes.egress = Some(bind.egress_info.clone());
                                match r.0 {
                                    Ok(ups_stream) => {
                                        self.stats.tcp.add_connection_established();
                                        let local_addr = ups_stream
                                            .local_addr()
                                            .map_err(TcpConnectError::SetupSocketFailed)?;
                                        tcp_notes.local = Some(local_addr);
                                        tcp_notes.chained.target_addr = Some(peer_addr);
                                        tcp_notes.chained.outgoing_addr = Some(local_addr);
                                        return Ok((ups_stream, bind));
                                    }
                                    Err(e) => {
                                        EscapeLogForTcpConnect {
                                            tcp_notes,
                                            task_id: &task_notes.id,
                                        }
                                        .log(&self.escape_logger, &e);
                                        // TODO tell resolver to remove addr
                                        returned_err = e;
                                        spawn_new_connection = true;
                                    }
                                }
                            }
                            Some(Err(r)) => {
                                running_connection -= 1;
                                if r.is_panic() {
                                    return Err(TcpConnectError::InternalServerError("connect task panic"));
                                }
                                spawn_new_connection = true;
                            }
                            None => unreachable!(),
                        }
                    }
                    _ = connect_interval.tick() => {
                        if skip_first_tick {
                            skip_first_tick = false;
                        } else {
                            spawn_new_connection = true;
                        }
                    }
                    r = resolver_job.get_r2_or_never(max_tries_each_family) => {
                        resolver_r2_done = true;
                        if let Ok(ips2) = r {
                            self.merge_ip_list(tcp_notes.tries, &mut ips, ips2);
                        }
                    }
                }
            } else if resolver_r2_done {
                tcp_notes.duration = instant_now.elapsed();
                return Err(returned_err);
            } else {
                match tokio::time::timeout(
                    self.config.happy_eyeballs.second_resolution_timeout(),
                    resolver_job.get_r2_or_never(max_tries_each_family),
                )
                .await
                {
                    Ok(Ok(ips2)) => {
                        resolver_r2_done = true;
                        self.merge_ip_list(tcp_notes.tries, &mut ips, ips2);
                        spawn_new_connection = true;
                    }
                    Ok(Err(_e)) => {
                        tcp_notes.duration = instant_now.elapsed();
                        return Err(returned_err);
                    }
                    Err(_) => {
                        tcp_notes.duration = instant_now.elapsed();
                        return Err(TcpConnectError::TimeoutByRule);
                    }
                }
            }
        }
    }

    pub(super) async fn tcp_connect_to<'a>(
        &'a self,
        tcp_notes: &'a mut TcpConnectTaskNotes,
        task_notes: &'a ServerTaskNotes,
    ) -> Result<(TcpStream, DirectFloatBindIp), TcpConnectError> {
        let mut tcp_connect_config = self.config.general.tcp_connect;

        let (keepalive, misc_opts) = if let Some(user_ctx) = task_notes.user_ctx() {
            let user = user_ctx.user();

            if let Some(user_config) = &user.config.tcp_connect {
                tcp_connect_config.limit_to(user_config);
            }

            let keepalive = self
                .config
                .tcp_keepalive
                .adjust_to(user.config.tcp_remote_keepalive);
            let misc_opts = user.config.tcp_remote_misc_opts(&self.config.tcp_misc_opts);
            (keepalive, misc_opts)
        } else {
            (self.config.tcp_keepalive, self.config.tcp_misc_opts)
        };

        match tcp_notes.upstream.host() {
            Host::Ip(ip) => {
                self.fixed_try_connect(
                    *ip,
                    tcp_connect_config,
                    keepalive,
                    misc_opts,
                    tcp_notes,
                    task_notes,
                )
                .await
            }
            Host::Domain(domain) => {
                let resolver_job =
                    self.resolve_happy(domain, self.get_resolve_strategy(task_notes), task_notes)?;

                self.happy_try_connect(
                    resolver_job,
                    tcp_connect_config,
                    keepalive,
                    misc_opts,
                    tcp_notes,
                    task_notes,
                )
                .await
            }
        }
    }

    pub(super) async fn tcp_connect_to_again<'a>(
        &'a self,
        new_tcp_notes: &'a mut TcpConnectTaskNotes,
        old_tcp_notes: &'a TcpConnectTaskNotes,
        task_notes: &'a ServerTaskNotes,
    ) -> Result<(TcpStream, DirectFloatBindIp), TcpConnectError> {
        new_tcp_notes.bind = old_tcp_notes.bind;

        let mut tcp_connect_config = self.config.general.tcp_connect;

        let misc_opts = if let Some(user_ctx) = task_notes.user_ctx() {
            if let Some(user_config) = &user_ctx.user().config.tcp_connect {
                tcp_connect_config.limit_to(user_config);
            }

            user_ctx
                .user()
                .config
                .tcp_remote_misc_opts(&self.config.tcp_misc_opts)
        } else {
            self.config.tcp_misc_opts
        };

        // tcp keepalive is not needed for ftp transfer connection as it shouldn't be idle
        let keepalive = TcpKeepAliveConfig::default();

        if new_tcp_notes.upstream.host_eq(&old_tcp_notes.upstream) {
            let control_addr = old_tcp_notes.next.ok_or_else(|| {
                TcpConnectError::SetupSocketFailed(io::Error::new(
                    io::ErrorKind::InvalidInput,
                    "no peer address for referenced connection found",
                ))
            })?;

            self.fixed_try_connect(
                control_addr.ip(),
                tcp_connect_config,
                keepalive,
                misc_opts,
                new_tcp_notes,
                task_notes,
            )
            .await
        } else {
            match new_tcp_notes.upstream.host() {
                Host::Ip(ip) => {
                    self.fixed_try_connect(
                        *ip,
                        tcp_connect_config,
                        keepalive,
                        misc_opts,
                        new_tcp_notes,
                        task_notes,
                    )
                    .await
                }
                Host::Domain(domain) => {
                    let mut resolve_strategy = self.get_resolve_strategy(task_notes);
                    match new_tcp_notes.bind {
                        Some(IpAddr::V4(_)) => resolve_strategy.query_v4only(),
                        Some(IpAddr::V6(_)) => resolve_strategy.query_v6only(),
                        None => {}
                    }

                    let resolver_job = self.resolve_happy(domain, resolve_strategy, task_notes)?;
                    self.happy_try_connect(
                        resolver_job,
                        tcp_connect_config,
                        keepalive,
                        misc_opts,
                        new_tcp_notes,
                        task_notes,
                    )
                    .await
                }
            }
        }
    }

    pub(super) async fn tcp_new_connection<'a>(
        &'a self,
        tcp_notes: &'a mut TcpConnectTaskNotes,
        task_notes: &'a ServerTaskNotes,
        task_stats: ArcTcpConnectionTaskRemoteStats,
    ) -> TcpConnectResult {
        let (stream, _) = self.tcp_connect_to(tcp_notes, task_notes).await?;
        let (r, w) = stream.into_split();

        let mut wrapper_stats = DirectTcpMixedRemoteStats::new(&self.stats, task_stats);
        wrapper_stats.push_user_io_stats(self.fetch_user_upstream_io_stats(task_notes));
        let (ups_r_stats, ups_w_stats) = wrapper_stats.into_pair();

        let limit_config = &self.config.general.tcp_sock_speed_limit;
        let r = LimitedReader::new(
            r,
            limit_config.shift_millis,
            limit_config.max_south,
            ups_r_stats,
        );
        let w = LimitedWriter::new(
            w,
            limit_config.shift_millis,
            limit_config.max_north,
            ups_w_stats,
        );

        Ok((Box::new(r), Box::new(w)))
    }
}
