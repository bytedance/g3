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
use std::sync::Arc;

use tokio::io::{AsyncRead, AsyncWrite};
use tokio::net::UdpSocket;

use g3_daemon::stat::remote::{
    ArcTcpConnectionTaskRemoteStats, TcpConnectionTaskRemoteStatsWrapper,
};
use g3_io_ext::{AsyncStream, LimitedReader, LimitedWriter};
use g3_openssl::SslStream;
use g3_socket::BindAddr;
use g3_socks::v5;
use g3_types::net::{SocketBufferConfig, UpstreamAddr};

use super::{ProxyFloatEscaper, ProxyFloatSocks5sPeer};
use crate::log::escape::tls_handshake::TlsApplication;
use crate::module::tcp_connect::{
    TcpConnectError, TcpConnectResult, TcpConnectTaskConf, TcpConnectTaskNotes, TlsConnectTaskConf,
};
use crate::serve::ServerTaskNotes;

impl ProxyFloatSocks5sPeer {
    async fn socks5_connect_tcp_connect_to(
        &self,
        escaper: &ProxyFloatEscaper,
        task_conf: &TcpConnectTaskConf<'_>,
        tcp_notes: &mut TcpConnectTaskNotes,
        task_notes: &ServerTaskNotes,
    ) -> Result<SslStream<impl AsyncRead + AsyncWrite + use<>>, TcpConnectError> {
        let mut stream = escaper
            .tls_handshake_with_peer(task_conf, tcp_notes, task_notes, &self.tls_name, self)
            .await?;
        let outgoing_addr = v5::client::socks5_connect_to(
            &mut stream,
            &self.shared_config.auth_info,
            task_conf.upstream,
        )
        .await?;
        // no need to replace the ip with registered public address.
        // prefer to use the one returned directly by remote proxy
        tcp_notes.chained.outgoing_addr = Some(outgoing_addr);
        // we can not determine the real upstream addr that the proxy choose to connect to

        Ok(stream)
    }

    pub(super) async fn timed_socks5_connect_tcp_connect_to(
        &self,
        escaper: &ProxyFloatEscaper,
        task_conf: &TcpConnectTaskConf<'_>,
        tcp_notes: &mut TcpConnectTaskNotes,
        task_notes: &ServerTaskNotes,
    ) -> Result<SslStream<impl AsyncRead + AsyncWrite + use<>>, TcpConnectError> {
        tokio::time::timeout(
            escaper.config.peer_negotiation_timeout,
            self.socks5_connect_tcp_connect_to(escaper, task_conf, tcp_notes, task_notes),
        )
        .await
        .map_err(|_| TcpConnectError::NegotiationPeerTimeout)?
    }

    /// setup udp associate with remote proxy
    /// return (socket, listen_addr, peer_addr)
    async fn socks5_udp_associate(
        &self,
        escaper: &ProxyFloatEscaper,
        buf_conf: SocketBufferConfig,
        tcp_notes: &mut TcpConnectTaskNotes,
        task_notes: &ServerTaskNotes,
    ) -> Result<
        (
            SslStream<impl AsyncRead + AsyncWrite + use<>>,
            UdpSocket,
            SocketAddr,
            SocketAddr,
        ),
        io::Error,
    > {
        let tcp_task_conf = TcpConnectTaskConf {
            upstream: &UpstreamAddr::empty(),
        };
        let mut ctl_stream = escaper
            .tls_handshake_with_peer(&tcp_task_conf, tcp_notes, task_notes, &self.tls_name, self)
            .await
            .map_err(io::Error::other)?;
        let local_tcp_addr = tcp_notes
            .local
            .ok_or_else(|| io::Error::other("no local tcp address"))?;
        let peer_tcp_addr = tcp_notes
            .next
            .ok_or_else(|| io::Error::other("no peer tcp address"))?;

        // bind early and send listen_addr if configured ?
        let send_udp_ip = match local_tcp_addr.ip() {
            IpAddr::V4(_) => IpAddr::V4(Ipv4Addr::UNSPECIFIED),
            IpAddr::V6(_) => IpAddr::V6(Ipv6Addr::UNSPECIFIED),
        };
        let send_udp_addr = SocketAddr::new(send_udp_ip, 0);

        let peer_udp_addr = v5::client::socks5_udp_associate(
            &mut ctl_stream,
            &self.shared_config.auth_info,
            send_udp_addr,
        )
        .await
        .map_err(io::Error::other)?;
        let peer_udp_addr = self.transmute_udp_peer_addr(peer_udp_addr, peer_tcp_addr.ip());
        let socket = g3_socket::udp::new_std_socket_to(
            peer_udp_addr,
            &BindAddr::Ip(local_tcp_addr.ip()),
            buf_conf,
            escaper.config.udp_misc_opts,
        )?;
        socket.connect(peer_udp_addr)?;
        let socket = UdpSocket::from_std(socket)?;
        let listen_addr = socket.local_addr()?;

        Ok((ctl_stream, socket, listen_addr, peer_udp_addr))
    }

    pub(super) async fn timed_socks5_udp_associate(
        &self,
        escaper: &ProxyFloatEscaper,
        buf_conf: SocketBufferConfig,
        tcp_notes: &mut TcpConnectTaskNotes,
        task_notes: &ServerTaskNotes,
    ) -> Result<
        (
            SslStream<impl AsyncRead + AsyncWrite + use<>>,
            UdpSocket,
            SocketAddr,
            SocketAddr,
        ),
        io::Error,
    > {
        tokio::time::timeout(
            escaper.config.peer_negotiation_timeout,
            self.socks5_udp_associate(escaper, buf_conf, tcp_notes, task_notes),
        )
        .await
        .map_err(|_| io::Error::new(io::ErrorKind::TimedOut, "peer negotiation timeout"))?
    }

    pub(super) async fn socks5_new_tcp_connection(
        &self,
        escaper: &ProxyFloatEscaper,
        task_conf: &TcpConnectTaskConf<'_>,
        tcp_notes: &mut TcpConnectTaskNotes,
        task_notes: &ServerTaskNotes,
        task_stats: ArcTcpConnectionTaskRemoteStats,
    ) -> TcpConnectResult {
        let ups_s = self
            .timed_socks5_connect_tcp_connect_to(escaper, task_conf, tcp_notes, task_notes)
            .await?;

        // add task and user stats
        let mut wrapper_stats = TcpConnectionTaskRemoteStatsWrapper::new(task_stats);
        wrapper_stats.push_other_stats(escaper.fetch_user_upstream_io_stats(task_notes));
        let wrapper_stats = Arc::new(wrapper_stats);

        let (r, w) = ups_s.into_split();
        let r = LimitedReader::new(r, wrapper_stats.clone());
        let w = LimitedWriter::new(w, wrapper_stats);

        Ok((Box::new(r), Box::new(w)))
    }

    pub(super) async fn socks5_connect_tls_connect_to(
        &self,
        escaper: &ProxyFloatEscaper,
        task_conf: &TlsConnectTaskConf<'_>,
        tcp_notes: &mut TcpConnectTaskNotes,
        task_notes: &ServerTaskNotes,
        tls_application: TlsApplication,
    ) -> Result<SslStream<impl AsyncRead + AsyncWrite + use<>>, TcpConnectError> {
        let ups_s = self
            .timed_socks5_connect_tcp_connect_to(escaper, &task_conf.tcp, tcp_notes, task_notes)
            .await?;
        escaper
            .tls_connect_over_tunnel(ups_s, task_conf, tcp_notes, task_notes, tls_application)
            .await
    }

    pub(super) async fn socks5_new_tls_connection(
        &self,
        escaper: &ProxyFloatEscaper,
        task_conf: &TlsConnectTaskConf<'_>,
        tcp_notes: &mut TcpConnectTaskNotes,
        task_notes: &ServerTaskNotes,
        task_stats: ArcTcpConnectionTaskRemoteStats,
    ) -> TcpConnectResult {
        let ups_s = self
            .timed_socks5_connect_tcp_connect_to(escaper, &task_conf.tcp, tcp_notes, task_notes)
            .await?;
        escaper
            .new_tls_connection_over_tunnel(ups_s, task_conf, tcp_notes, task_notes, task_stats)
            .await
    }
}
