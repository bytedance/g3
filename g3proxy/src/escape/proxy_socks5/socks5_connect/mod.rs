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
use std::net::{IpAddr, Ipv4Addr, Ipv6Addr, SocketAddr};
use std::pin::Pin;

use anyhow::anyhow;
use tokio::io::AsyncReadExt;
use tokio::net::{tcp, UdpSocket};
use tokio::sync::oneshot;
use tokio_openssl::SslStream;

use g3_daemon::stat::remote::{
    ArcTcpConnectionTaskRemoteStats, TcpConnectionTaskRemoteStatsWrapper,
};
use g3_io_ext::{AggregatedIo, LimitedReader, LimitedWriter};
use g3_socks::v5;
use g3_types::net::{OpensslTlsClientConfig, SocketBufferConfig};

use super::{ProxySocks5Escaper, ProxySocks5EscaperStats};
use crate::log::escape::tls_handshake::{EscapeLogForTlsHandshake, TlsApplication};
use crate::module::tcp_connect::{TcpConnectError, TcpConnectResult, TcpConnectTaskNotes};
use crate::serve::ServerTaskNotes;

mod stats;
use stats::ProxyTcpMixedRemoteStats;

impl ProxySocks5Escaper {
    pub(super) async fn socks5_connect_tcp_connect_to<'a>(
        &'a self,
        tcp_notes: &'a mut TcpConnectTaskNotes,
        task_notes: &'a ServerTaskNotes,
    ) -> Result<
        (
            LimitedReader<tcp::OwnedReadHalf>,
            LimitedWriter<tcp::OwnedWriteHalf>,
        ),
        TcpConnectError,
    > {
        let (mut r, mut w) = self.tcp_new_connection(tcp_notes, task_notes).await?;
        let outgoing_addr = v5::client::socks5_connect_to(
            &mut r,
            &mut w,
            &self.config.auth_info,
            &tcp_notes.upstream,
        )
        .await?;
        tcp_notes.chained.outgoing_addr = Some(outgoing_addr);
        // we can not determine the real upstream addr that the proxy choose to connect to

        Ok((r, w))
    }

    pub(super) async fn timed_socks5_connect_tcp_connect_to<'a>(
        &'a self,
        tcp_notes: &'a mut TcpConnectTaskNotes,
        task_notes: &'a ServerTaskNotes,
    ) -> Result<
        (
            LimitedReader<tcp::OwnedReadHalf>,
            LimitedWriter<tcp::OwnedWriteHalf>,
        ),
        TcpConnectError,
    > {
        tokio::time::timeout(
            self.config.peer_negotiation_timeout,
            self.socks5_connect_tcp_connect_to(tcp_notes, task_notes),
        )
        .await
        .map_err(|_| TcpConnectError::NegotiationPeerTimeout)?
    }

    /// setup udp associate with remote proxy
    /// return (socket, listen_addr, peer_addr)
    pub(super) async fn socks5_udp_associate(
        &self,
        buf_conf: SocketBufferConfig,
        tcp_notes: &mut TcpConnectTaskNotes,
        task_notes: &ServerTaskNotes,
    ) -> Result<
        (
            oneshot::Receiver<Option<io::Error>>,
            UdpSocket,
            SocketAddr,
            SocketAddr,
        ),
        io::Error,
    > {
        let (mut r, mut w) = self
            .tcp_new_connection(tcp_notes, task_notes)
            .await
            .map_err(|e| io::Error::new(io::ErrorKind::Other, e))?;
        let local_tcp_addr = tcp_notes
            .local
            .ok_or_else(|| io::Error::new(io::ErrorKind::Other, "no local tcp address"))?;

        // bind early and send listen_addr if configured ?
        let send_udp_ip = match local_tcp_addr.ip() {
            IpAddr::V4(_) => IpAddr::V4(Ipv4Addr::UNSPECIFIED),
            IpAddr::V6(_) => IpAddr::V6(Ipv6Addr::UNSPECIFIED),
        };
        let send_udp_addr = SocketAddr::new(send_udp_ip, 0);

        let peer_addr =
            v5::client::socks5_udp_associate(&mut r, &mut w, &self.config.auth_info, send_udp_addr)
                .await
                .map_err(|e| io::Error::new(io::ErrorKind::Other, e))?;
        let socket = g3_socket::udp::new_std_socket_to(
            peer_addr,
            Some(local_tcp_addr.ip()),
            buf_conf,
            &self.config.udp_misc_opts,
        )?;
        let socket = UdpSocket::from_std(socket)?;
        socket.connect(peer_addr).await?;
        let listen_addr = socket.local_addr()?;

        let r = r.into_inner();
        let w = w.into_inner();
        let stream = r
            .reunite(w)
            .map_err(|e| io::Error::new(io::ErrorKind::Other, e))?;

        let (mut tcp_close_sender, tcp_close_receiver) = oneshot::channel::<Option<io::Error>>();
        tokio::spawn(async move {
            let mut tcp_stream = stream;
            let mut buf = [0u8; 4];

            tokio::select! {
                biased;

                r = tcp_stream.read(&mut buf) => {
                    let e = match r {
                        Ok(0) => None,
                        Ok(_) => Some(io::Error::new(
                            io::ErrorKind::Other,
                            "unexpected data received in the tcp connection",
                        )),
                        Err(e) => Some(e),
                    };
                    let _ = tcp_close_sender.send(e);
                }
                _ = tcp_close_sender.closed() => {}
            }
        });

        Ok((tcp_close_receiver, socket, listen_addr, peer_addr))
    }

    pub(super) async fn timed_socks5_udp_associate(
        &self,
        buf_conf: SocketBufferConfig,
        tcp_notes: &mut TcpConnectTaskNotes,
        task_notes: &ServerTaskNotes,
    ) -> Result<
        (
            oneshot::Receiver<Option<io::Error>>,
            UdpSocket,
            SocketAddr,
            SocketAddr,
        ),
        io::Error,
    > {
        tokio::time::timeout(
            self.config.peer_negotiation_timeout,
            self.socks5_udp_associate(buf_conf, tcp_notes, task_notes),
        )
        .await
        .map_err(|_| io::Error::new(io::ErrorKind::TimedOut, "peer negotiation timeout"))?
    }

    pub(super) async fn socks5_new_tcp_connection<'a>(
        &'a self,
        tcp_notes: &'a mut TcpConnectTaskNotes,
        task_notes: &'a ServerTaskNotes,
        task_stats: ArcTcpConnectionTaskRemoteStats,
    ) -> TcpConnectResult {
        let (mut r, mut w) = self
            .timed_socks5_connect_tcp_connect_to(tcp_notes, task_notes)
            .await?;

        let mut wrapper_stats = ProxyTcpMixedRemoteStats::new(&self.stats, task_stats);
        wrapper_stats.push_user_io_stats(self.fetch_user_upstream_io_stats(task_notes));
        let (ups_r_stats, ups_w_stats) = wrapper_stats.into_pair();

        r.reset_stats(ups_r_stats);
        w.reset_stats(ups_w_stats);

        Ok((Box::new(r), Box::new(w)))
    }

    pub(super) async fn socks5_connect_tls_connect_to<'a>(
        &'a self,
        tcp_notes: &'a mut TcpConnectTaskNotes,
        task_notes: &'a ServerTaskNotes,
        tls_config: &'a OpensslTlsClientConfig,
        tls_name: &'a str,
        tls_application: TlsApplication,
    ) -> Result<
        SslStream<
            AggregatedIo<LimitedReader<tcp::OwnedReadHalf>, LimitedWriter<tcp::OwnedWriteHalf>>,
        >,
        TcpConnectError,
    > {
        let (ups_r, ups_w) = self
            .timed_socks5_connect_tcp_connect_to(tcp_notes, task_notes)
            .await?;

        let ssl = tls_config
            .build_ssl(tls_name, tcp_notes.upstream.port())
            .map_err(TcpConnectError::InternalTlsClientError)?;
        let mut stream = SslStream::new(
            ssl,
            AggregatedIo {
                reader: ups_r,
                writer: ups_w,
            },
        )
        .map_err(|e| TcpConnectError::InternalTlsClientError(anyhow::Error::new(e)))?;

        match tokio::time::timeout(
            tls_config.handshake_timeout,
            Pin::new(&mut stream).connect(),
        )
        .await
        {
            Ok(Ok(_)) => Ok(stream),
            Ok(Err(e)) => {
                let e = anyhow::Error::new(e);
                EscapeLogForTlsHandshake {
                    tcp_notes,
                    task_id: &task_notes.id,
                    tls_name,
                    tls_peer: &tcp_notes.upstream,
                    tls_application,
                }
                .log(&self.escape_logger, &e);
                Err(TcpConnectError::UpstreamTlsHandshakeFailed(e))
            }
            Err(_) => {
                let e = anyhow!("upstream tls handshake timed out");
                EscapeLogForTlsHandshake {
                    tcp_notes,
                    task_id: &task_notes.id,
                    tls_name,
                    tls_peer: &tcp_notes.upstream,
                    tls_application,
                }
                .log(&self.escape_logger, &e);
                Err(TcpConnectError::UpstreamTlsHandshakeTimeout)
            }
        }
    }

    pub(super) async fn socks5_new_tls_connection<'a>(
        &'a self,
        tcp_notes: &'a mut TcpConnectTaskNotes,
        task_notes: &'a ServerTaskNotes,
        task_stats: ArcTcpConnectionTaskRemoteStats,
        tls_config: &'a OpensslTlsClientConfig,
        tls_name: &'a str,
    ) -> TcpConnectResult {
        let tls_stream = self
            .socks5_connect_tls_connect_to(
                tcp_notes,
                task_notes,
                tls_config,
                tls_name,
                TlsApplication::TcpStream,
            )
            .await?;

        let (ups_r, ups_w) = tokio::io::split(tls_stream);

        // add task and user stats
        let mut wrapper_stats = TcpConnectionTaskRemoteStatsWrapper::new(task_stats);
        wrapper_stats.push_other_stats(self.fetch_user_upstream_io_stats(task_notes));
        let (ups_r_stats, ups_w_stats) = wrapper_stats.into_pair();

        let ups_r = LimitedReader::new_unlimited(ups_r, ups_r_stats);
        let ups_w = LimitedWriter::new_unlimited(ups_w, ups_w_stats);

        Ok((Box::new(ups_r), Box::new(ups_w)))
    }
}
