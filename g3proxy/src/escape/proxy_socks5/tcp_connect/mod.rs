/*
 * SPDX-License-Identifier: Apache-2.0
 * Copyright 2023-2025 ByteDance and/or its affiliates.
 */

use std::net::{IpAddr, SocketAddr};

use tokio::net::{TcpSocket, TcpStream};
use tokio::task::JoinSet;
use tokio::time::Instant;

use g3_io_ext::LimitedStream;
use g3_socket::BindAddr;
use g3_types::net::{ConnectError, Host};

use super::ProxySocks5Escaper;
use crate::log::escape::tcp_connect::EscapeLogForTcpConnect;
use crate::module::tcp_connect::{TcpConnectError, TcpConnectTaskConf, TcpConnectTaskNotes};
use crate::resolve::HappyEyeballsResolveJob;
use crate::serve::ServerTaskNotes;

impl ProxySocks5Escaper {
    fn prepare_connect_socket(
        &self,
        peer_ip: IpAddr,
    ) -> Result<(TcpSocket, BindAddr), TcpConnectError> {
        let bind_ip = match peer_ip {
            IpAddr::V4(_) => {
                if self.config.no_ipv4 {
                    return Err(TcpConnectError::ForbiddenAddressFamily);
                }
                self.config.bind_v4.map(IpAddr::V4)
            }
            IpAddr::V6(_) => {
                if self.config.no_ipv6 {
                    return Err(TcpConnectError::ForbiddenAddressFamily);
                }
                self.config.bind_v6.map(IpAddr::V6)
            }
        };

        #[cfg(any(
            target_os = "linux",
            target_os = "android",
            target_os = "macos",
            target_os = "illumos",
            target_os = "solaris"
        ))]
        let bind = bind_ip.map(BindAddr::Ip).unwrap_or_else(|| {
            self.config
                .bind_interface
                .map(BindAddr::Interface)
                .unwrap_or_default()
        });
        #[cfg(not(any(
            target_os = "linux",
            target_os = "android",
            target_os = "macos",
            target_os = "illumos",
            target_os = "solaris"
        )))]
        let bind = bind_ip.map(BindAddr::Ip).unwrap_or_default();
        let sock = g3_socket::tcp::new_socket_to(
            peer_ip,
            &bind,
            &self.config.tcp_keepalive,
            &self.config.tcp_misc_opts,
            true,
        )
        .map_err(TcpConnectError::SetupSocketFailed)?;
        Ok((sock, bind))
    }

    async fn fixed_try_connect(
        &self,
        peer: SocketAddr,
        task_conf: &TcpConnectTaskConf<'_>,
        tcp_notes: &mut TcpConnectTaskNotes,
        task_notes: &ServerTaskNotes,
    ) -> Result<TcpStream, TcpConnectError> {
        let (sock, bind) = self.prepare_connect_socket(peer.ip())?;
        tcp_notes.next = Some(peer);
        tcp_notes.bind = bind;

        let instant_now = Instant::now();

        self.stats.tcp.connect.add_attempted();
        tcp_notes.tries = 1;
        match tokio::time::timeout(
            self.config.general.tcp_connect.each_timeout(),
            sock.connect(peer),
        )
        .await
        {
            Ok(Ok(ups_stream)) => {
                self.stats.tcp.connect.add_success();
                tcp_notes.duration = instant_now.elapsed();

                let local_addr = ups_stream
                    .local_addr()
                    .map_err(TcpConnectError::SetupSocketFailed)?;
                self.stats.tcp.connect.add_established();
                tcp_notes.local = Some(local_addr);
                // the chained outgoing addr is not detected at here
                Ok(ups_stream)
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
        peer_port: u16,
        task_conf: &TcpConnectTaskConf<'_>,
        tcp_notes: &mut TcpConnectTaskNotes,
        task_notes: &ServerTaskNotes,
    ) -> Result<TcpStream, TcpConnectError> {
        let max_tries_each_family = self.config.general.tcp_connect.max_tries();
        let mut ips = resolver_job
            .get_r1_or_first(
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
        let each_timeout = self.config.general.tcp_connect.each_timeout();

        tcp_notes.tries = 0;
        let instant_now = Instant::now();
        let mut returned_err = TcpConnectError::NoAddressConnected;

        loop {
            if spawn_new_connection {
                if let Some(ip) = ips.pop() {
                    let (sock, bind) = self.prepare_connect_socket(ip)?;
                    let peer = SocketAddr::new(ip, peer_port);
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
                                tcp_notes.next = Some(peer_addr);
                                tcp_notes.bind = r.2;
                                match r.0 {
                                    Ok(ups_stream) => {
                                        let local_addr = ups_stream
                                            .local_addr()
                                            .map_err(TcpConnectError::SetupSocketFailed)?;
                                        self.stats.tcp.connect.add_established();
                                        tcp_notes.local = Some(local_addr);
                                        // the chained outgoing addr is not detected at here
                                        return Ok(ups_stream);
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

    async fn tcp_connect_to(
        &self,
        task_conf: &TcpConnectTaskConf<'_>,
        tcp_notes: &mut TcpConnectTaskNotes,
        task_notes: &ServerTaskNotes,
    ) -> Result<TcpStream, TcpConnectError> {
        let peer_proxy = self.get_next_proxy(task_notes, task_conf.upstream.host());

        match peer_proxy.host() {
            Host::Ip(ip) => {
                self.fixed_try_connect(
                    SocketAddr::new(*ip, peer_proxy.port()),
                    task_conf,
                    tcp_notes,
                    task_notes,
                )
                .await
            }
            Host::Domain(domain) => {
                let resolver_job = self.resolve_happy(domain.clone())?;

                self.happy_try_connect(
                    resolver_job,
                    peer_proxy.port(),
                    task_conf,
                    tcp_notes,
                    task_notes,
                )
                .await
            }
        }
    }

    pub(super) async fn tcp_new_connection(
        &self,
        task_conf: &TcpConnectTaskConf<'_>,
        tcp_notes: &mut TcpConnectTaskNotes,
        task_notes: &ServerTaskNotes,
    ) -> Result<LimitedStream<TcpStream>, TcpConnectError> {
        let stream = self
            .tcp_connect_to(task_conf, tcp_notes, task_notes)
            .await?;

        let limit_config = &self.config.general.tcp_sock_speed_limit;
        let stream = LimitedStream::local_limited(
            stream,
            limit_config.shift_millis,
            limit_config.max_south,
            limit_config.max_north,
            self.stats.clone(),
        );

        Ok(stream)
    }
}
