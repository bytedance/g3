/*
 * SPDX-License-Identifier: Apache-2.0
 * Copyright 2023-2025 ByteDance and/or its affiliates.
 */

use std::borrow::Cow;
use std::io;
use std::net::{IpAddr, SocketAddr};
use std::sync::Arc;

use tokio::net::{TcpSocket, TcpStream};
use tokio::task::JoinSet;
use tokio::time::Instant;

use g3_daemon::stat::remote::ArcTcpConnectionTaskRemoteStats;
use g3_io_ext::{LimitedReader, LimitedWriter};
use g3_socket::BindAddr;
use g3_socket::util::AddressFamily;
use g3_types::acl::AclAction;
use g3_types::net::{ConnectError, Host, TcpKeepAliveConfig, UpstreamAddr};

use super::{DirectFloatBindIp, DirectFloatEscaper};
use crate::escape::direct_fixed::tcp_connect::DirectTcpConnectConfig;
use crate::log::escape::tcp_connect::EscapeLogForTcpConnect;
use crate::module::tcp_connect::{
    TcpConnectError, TcpConnectRemoteWrapperStats, TcpConnectResult, TcpConnectTaskConf,
    TcpConnectTaskNotes,
};
use crate::resolve::HappyEyeballsResolveJob;
use crate::serve::ServerTaskNotes;

impl DirectFloatEscaper {
    fn handle_tcp_target_ip_acl_action(
        &self,
        action: AclAction,
        task_notes: &ServerTaskNotes,
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
        bind: BindAddr,
        task_notes: &ServerTaskNotes,
        config: &DirectTcpConnectConfig<'_>,
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

        let bind = if let Some(ip) = bind.ip() {
            self.select_bind_again(ip, task_notes)
                .map_err(TcpConnectError::EscaperNotUsable)?
        } else {
            self.select_bind(AddressFamily::from(&peer_ip), task_notes)
                .map_err(TcpConnectError::EscaperNotUsable)?
        };

        let sock = g3_socket::tcp::new_socket_to(
            peer_ip,
            &BindAddr::Ip(bind.ip),
            &config.keepalive,
            &config.misc_opts,
            true,
        )
        .map_err(TcpConnectError::SetupSocketFailed)?;
        Ok((sock, bind))
    }

    async fn fixed_try_connect(
        &self,
        peer_ip: IpAddr,
        config: DirectTcpConnectConfig<'_>,
        task_conf: &TcpConnectTaskConf<'_>,
        tcp_notes: &mut TcpConnectTaskNotes,
        task_notes: &ServerTaskNotes,
    ) -> Result<(TcpStream, DirectFloatBindIp), TcpConnectError> {
        let (sock, bind) =
            self.prepare_connect_socket(peer_ip, tcp_notes.bind, task_notes, &config)?;
        let peer = SocketAddr::new(peer_ip, task_conf.upstream.port());
        tcp_notes.next = Some(peer);
        tcp_notes.bind = BindAddr::Ip(bind.ip);
        tcp_notes.expire = bind.expire_datetime;
        tcp_notes.egress = Some(bind.egress_info.clone());

        let instant_now = Instant::now();

        self.stats.tcp.connect.add_attempted();
        tcp_notes.tries = 1;
        match tokio::time::timeout(config.connect.each_timeout(), sock.connect(peer)).await {
            Ok(Ok(ups_stream)) => {
                self.stats.tcp.connect.add_success();
                tcp_notes.duration = instant_now.elapsed();

                let local_addr = ups_stream
                    .local_addr()
                    .map_err(TcpConnectError::SetupSocketFailed)?;
                self.stats.tcp.connect.add_established();
                tcp_notes.local = Some(local_addr);
                tcp_notes.chained.target_addr = Some(peer);
                tcp_notes.chained.outgoing_addr = Some(local_addr);
                Ok((ups_stream, bind))
            }
            Ok(Err(e)) => {
                self.stats.tcp.connect.add_error();
                tcp_notes.duration = instant_now.elapsed();

                let e = TcpConnectError::ConnectFailed(ConnectError::from(e));
                if let Some(logger) = &self.escape_logger {
                    EscapeLogForTcpConnect {
                        upstream: task_conf.upstream,
                        tcp_notes,
                        task_id: &task_notes.id,
                    }
                    .log(logger, &e);
                }
                Err(e)
            }
            Err(_) => {
                self.stats.tcp.connect.add_timeout();
                tcp_notes.duration = instant_now.elapsed();

                let e = TcpConnectError::TimeoutByRule;
                if let Some(logger) = &self.escape_logger {
                    EscapeLogForTcpConnect {
                        upstream: task_conf.upstream,
                        tcp_notes,
                        task_id: &task_notes.id,
                    }
                    .log(logger, &e);
                }
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
        config: DirectTcpConnectConfig<'_>,
        task_conf: &TcpConnectTaskConf<'_>,
        tcp_notes: &mut TcpConnectTaskNotes,
        task_notes: &ServerTaskNotes,
    ) -> Result<(TcpStream, DirectFloatBindIp), TcpConnectError> {
        let max_tries_each_family = config.connect.max_tries();
        let mut ips = resolver_job
            .get_r1_or_first_many(
                self.config.happy_eyeballs.resolution_delay(),
                max_tries_each_family,
            )
            .await?;

        let mut c_set = JoinSet::new();

        let mut connect_interval =
            tokio::time::interval(self.config.happy_eyeballs.connection_attempt_delay());
        // connect_interval.tick().await; will take 1ms
        // let's use local vars to skip the first tick()
        let mut skip_first_tick = true;

        let mut spawn_new_connection = true;
        let mut running_connection = 0;
        let mut resolver_r2_done = false;
        let each_timeout = config.connect.each_timeout();

        tcp_notes.tries = 0;
        let instant_now = Instant::now();
        let mut returned_err = TcpConnectError::NoAddressConnected;

        loop {
            if spawn_new_connection && let Some(ip) = ips.pop() {
                let (sock, bind) =
                    self.prepare_connect_socket(ip, tcp_notes.bind, task_notes, &config)?;
                let peer = SocketAddr::new(ip, task_conf.upstream.port());
                running_connection += 1;
                spawn_new_connection = false;
                tcp_notes.tries += 1;
                let stats = self.stats.clone();
                c_set.spawn(async move {
                    stats.tcp.connect.add_attempted();
                    match tokio::time::timeout(each_timeout, sock.connect(peer)).await {
                        Ok(Ok(stream)) => {
                            stats.tcp.connect.add_success();
                            (Ok(stream), peer, bind)
                        }
                        Ok(Err(e)) => {
                            stats.tcp.connect.add_error();
                            (
                                Err(TcpConnectError::ConnectFailed(ConnectError::from(e))),
                                peer,
                                bind,
                            )
                        }
                        Err(_) => {
                            stats.tcp.connect.add_timeout();
                            (Err(TcpConnectError::TimeoutByRule), peer, bind)
                        }
                    }
                });
                connect_interval.reset();
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
                                tcp_notes.bind = BindAddr::Ip(bind.ip);
                                tcp_notes.expire = bind.expire_datetime;
                                tcp_notes.egress = Some(bind.egress_info.clone());
                                match r.0 {
                                    Ok(ups_stream) => {
                                        let local_addr = ups_stream
                                            .local_addr()
                                            .map_err(TcpConnectError::SetupSocketFailed)?;
                                        self.stats.tcp.connect.add_established();
                                        tcp_notes.local = Some(local_addr);
                                        tcp_notes.chained.target_addr = Some(peer_addr);
                                        tcp_notes.chained.outgoing_addr = Some(local_addr);
                                        return Ok((ups_stream, bind));
                                    }
                                    Err(e) => {
                                        if let Some(logger) = &self.escape_logger {
                                            EscapeLogForTcpConnect {
                                                upstream: task_conf.upstream,
                                                tcp_notes,
                                                task_id: &task_notes.id,
                                            }
                                            .log(logger, &e);
                                        }
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

    pub(super) async fn tcp_connect_to(
        &self,
        task_conf: &TcpConnectTaskConf<'_>,
        tcp_notes: &mut TcpConnectTaskNotes,
        task_notes: &ServerTaskNotes,
    ) -> Result<(TcpStream, DirectFloatBindIp), TcpConnectError> {
        let mut config = DirectTcpConnectConfig {
            connect: self.config.general.tcp_connect,
            keepalive: self.config.tcp_keepalive,
            misc_opts: Cow::Borrowed(&self.config.tcp_misc_opts),
        };

        if let Some(user_ctx) = task_notes.user_ctx() {
            let user_config = user_ctx.user_config();

            if let Some(user_config) = &user_config.tcp_connect {
                config.connect.limit_to(user_config);
            }

            config.keepalive = config.keepalive.adjust_to(user_config.tcp_remote_keepalive);
            config.misc_opts = user_config.tcp_remote_misc_opts(&self.config.tcp_misc_opts);
        }

        match task_conf.upstream.host() {
            Host::Ip(ip) => {
                self.fixed_try_connect(*ip, config, task_conf, tcp_notes, task_notes)
                    .await
            }
            Host::Domain(domain) => {
                let resolver_job = self.resolve_happy(
                    domain.clone(),
                    self.get_resolve_strategy(task_notes),
                    task_notes,
                )?;

                self.happy_try_connect(resolver_job, config, task_conf, tcp_notes, task_notes)
                    .await
            }
        }
    }

    pub(super) async fn tcp_connect_to_again(
        &self,
        task_conf: &TcpConnectTaskConf<'_>,
        old_upstream: &UpstreamAddr,
        new_tcp_notes: &mut TcpConnectTaskNotes,
        old_tcp_notes: &TcpConnectTaskNotes,
        task_notes: &ServerTaskNotes,
    ) -> Result<(TcpStream, DirectFloatBindIp), TcpConnectError> {
        new_tcp_notes.bind = old_tcp_notes.bind;

        let mut config = DirectTcpConnectConfig {
            connect: self.config.general.tcp_connect,
            // tcp keepalive is not needed for ftp transfer connection as it shouldn't be idle
            keepalive: TcpKeepAliveConfig::default(),
            misc_opts: Cow::Borrowed(&self.config.tcp_misc_opts),
        };

        if let Some(user_ctx) = task_notes.user_ctx() {
            if let Some(user_config) = &user_ctx.user_config().tcp_connect {
                config.connect.limit_to(user_config);
            }

            config.misc_opts = user_ctx
                .user_config()
                .tcp_remote_misc_opts(&self.config.tcp_misc_opts);
        }

        if task_conf.upstream.host_eq(old_upstream) {
            let control_addr = old_tcp_notes.next.ok_or_else(|| {
                TcpConnectError::SetupSocketFailed(io::Error::new(
                    io::ErrorKind::InvalidInput,
                    "no peer address for referenced connection found",
                ))
            })?;

            self.fixed_try_connect(
                control_addr.ip(),
                config,
                task_conf,
                new_tcp_notes,
                task_notes,
            )
            .await
        } else {
            match task_conf.upstream.host() {
                Host::Ip(ip) => {
                    self.fixed_try_connect(*ip, config, task_conf, new_tcp_notes, task_notes)
                        .await
                }
                Host::Domain(domain) => {
                    let mut resolve_strategy = self.get_resolve_strategy(task_notes);
                    match new_tcp_notes.bind {
                        BindAddr::Ip(IpAddr::V4(_)) => resolve_strategy.query_v4only(),
                        BindAddr::Ip(IpAddr::V6(_)) => resolve_strategy.query_v6only(),
                        _ => {}
                    }

                    let resolver_job =
                        self.resolve_happy(domain.clone(), resolve_strategy, task_notes)?;
                    self.happy_try_connect(
                        resolver_job,
                        config,
                        task_conf,
                        new_tcp_notes,
                        task_notes,
                    )
                    .await
                }
            }
        }
    }

    pub(super) async fn tcp_new_connection(
        &self,
        task_conf: &TcpConnectTaskConf<'_>,
        tcp_notes: &mut TcpConnectTaskNotes,
        task_notes: &ServerTaskNotes,
        task_stats: ArcTcpConnectionTaskRemoteStats,
    ) -> TcpConnectResult {
        let (stream, _) = self
            .tcp_connect_to(task_conf, tcp_notes, task_notes)
            .await?;
        let (r, w) = stream.into_split();

        let mut wrapper_stats = TcpConnectRemoteWrapperStats::new(self.stats.clone(), task_stats);
        wrapper_stats.push_user_io_stats(self.fetch_user_upstream_io_stats(task_notes));
        let wrapper_stats = Arc::new(wrapper_stats);

        let limit_config = &self.config.general.tcp_sock_speed_limit;
        let r = LimitedReader::local_limited(
            r,
            limit_config.shift_millis,
            limit_config.max_south,
            wrapper_stats.clone(),
        );
        let w = LimitedWriter::local_limited(
            w,
            limit_config.shift_millis,
            limit_config.max_north,
            wrapper_stats,
        );

        Ok((Box::new(r), Box::new(w)))
    }
}
