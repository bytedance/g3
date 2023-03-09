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

use std::net::{IpAddr, SocketAddr};

use tokio::io::AsyncWriteExt;
use tokio::net::{tcp, TcpSocket, TcpStream};
use tokio::task::JoinSet;
use tokio::time::Instant;

use g3_io_ext::{LimitedReader, LimitedWriter};
use g3_types::net::{ConnectError, Host, ProxyProtocolEncoder, UpstreamAddr};

use super::ProxyHttpsEscaper;
use crate::log::escape::tcp_connect::EscapeLogForTcpConnect;
use crate::module::tcp_connect::{TcpConnectError, TcpConnectTaskNotes};
use crate::resolve::HappyEyeballsResolveJob;
use crate::serve::ServerTaskNotes;

impl ProxyHttpsEscaper {
    fn prepare_connect_socket(
        &self,
        peer_ip: IpAddr,
    ) -> Result<(TcpSocket, Option<IpAddr>), TcpConnectError> {
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

        let sock = g3_socket::tcp::new_socket_to(
            peer_ip,
            bind_ip,
            &self.config.tcp_keepalive,
            &self.config.tcp_misc_opts,
            true,
        )
        .map_err(TcpConnectError::SetupSocketFailed)?;
        Ok((sock, bind_ip))
    }

    async fn fixed_try_connect(
        &self,
        peer: SocketAddr,
        tcp_notes: &mut TcpConnectTaskNotes,
        task_notes: &ServerTaskNotes,
    ) -> Result<TcpStream, TcpConnectError> {
        let (sock, bind) = self.prepare_connect_socket(peer.ip())?;
        tcp_notes.next = Some(peer);
        tcp_notes.bind = bind;

        let instant_now = Instant::now();

        self.stats.tcp.add_connection_attempted();
        tcp_notes.tries = 1;
        match tokio::time::timeout(
            self.config.general.tcp_connect.each_timeout(),
            sock.connect(peer),
        )
        .await
        {
            Ok(Ok(ups_stream)) => {
                tcp_notes.duration = instant_now.elapsed();

                self.stats.tcp.add_connection_established();
                let local_addr = ups_stream
                    .local_addr()
                    .map_err(TcpConnectError::SetupSocketFailed)?;
                tcp_notes.local = Some(local_addr);
                // the chained outgoing addr is not detected at here
                Ok(ups_stream)
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
        peer_port: u16,
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
                                tcp_notes.next = Some(peer_addr);
                                tcp_notes.bind = r.2;
                                match r.0 {
                                    Ok(ups_stream) => {
                                        self.stats.tcp.add_connection_established();
                                        let local_addr = ups_stream
                                            .local_addr()
                                            .map_err(TcpConnectError::SetupSocketFailed)?;
                                        tcp_notes.local = Some(local_addr);
                                        // the chained outgoing addr is not detected at here
                                        return Ok(ups_stream);
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

    async fn tcp_connect_to<'a>(
        &'a self,
        tcp_notes: &'a mut TcpConnectTaskNotes,
        task_notes: &'a ServerTaskNotes,
    ) -> Result<(UpstreamAddr, TcpStream), TcpConnectError> {
        let peer_proxy = self
            .get_next_proxy(task_notes, tcp_notes.upstream.host())
            .clone();

        let stream = match peer_proxy.host() {
            Host::Ip(ip) => {
                self.fixed_try_connect(
                    SocketAddr::new(*ip, peer_proxy.port()),
                    tcp_notes,
                    task_notes,
                )
                .await?
            }
            Host::Domain(domain) => {
                let resolver_job = self.resolve_happy(domain)?;

                self.happy_try_connect(resolver_job, peer_proxy.port(), tcp_notes, task_notes)
                    .await?
            }
        };

        Ok((peer_proxy, stream))
    }

    pub(super) async fn tcp_new_connection<'a>(
        &'a self,
        tcp_notes: &'a mut TcpConnectTaskNotes,
        task_notes: &'a ServerTaskNotes,
    ) -> Result<
        (
            UpstreamAddr,
            LimitedReader<tcp::OwnedReadHalf>,
            LimitedWriter<tcp::OwnedWriteHalf>,
        ),
        TcpConnectError,
    > {
        let (peer, stream) = self.tcp_connect_to(tcp_notes, task_notes).await?;
        let (r, w) = stream.into_split();

        let limit_config = &self.config.general.tcp_sock_speed_limit;
        let r = LimitedReader::new(
            r,
            limit_config.shift_millis,
            limit_config.max_south,
            self.stats.for_limited_reader(),
        );
        let mut w = LimitedWriter::new(
            w,
            limit_config.shift_millis,
            limit_config.max_north,
            self.stats.for_limited_writer(),
        );

        if let Some(version) = self.config.use_proxy_protocol {
            let mut encoder = ProxyProtocolEncoder::new(version);
            let bytes = encoder
                .encode_tcp(task_notes.client_addr, task_notes.server_addr)
                .map_err(TcpConnectError::ProxyProtocolEncodeError)?;
            w.write_all(bytes)
                .await
                .map_err(TcpConnectError::ProxyProtocolWriteFailed)?;
        }

        Ok((peer, r, w))
    }
}
