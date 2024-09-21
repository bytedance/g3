/*
 * Copyright 2024 ByteDance and/or its affiliates.
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

use std::net::SocketAddr;

use tokio::net::TcpStream;
use tokio::time::Instant;

use g3_io_ext::LimitedStream;
use g3_socket::BindAddr;
use g3_types::net::ConnectError;

use super::{NextProxyPeer, ProxyFloatEscaper};
use crate::log::escape::tcp_connect::EscapeLogForTcpConnect;
use crate::module::tcp_connect::{TcpConnectError, TcpConnectTaskNotes};
use crate::serve::ServerTaskNotes;

impl ProxyFloatEscaper {
    async fn try_connect_tcp(
        &self,
        peer: SocketAddr,
        bind: &BindAddr,
    ) -> Result<TcpStream, TcpConnectError> {
        // use new socket every time, as we set bind_no_port
        let sock = g3_socket::tcp::new_socket_to(
            peer.ip(),
            bind,
            &self.config.tcp_keepalive,
            &self.config.tcp_misc_opts,
            true,
        )
        .map_err(TcpConnectError::SetupSocketFailed)?;
        self.stats.tcp.add_connection_attempted();
        match sock.connect(peer).await {
            Ok(ups_stream) => {
                self.stats.tcp.add_connection_established();
                Ok(ups_stream)
            }
            Err(e) => Err(TcpConnectError::ConnectFailed(ConnectError::from(e))),
        }
    }

    async fn tcp_connect_to<P: NextProxyPeer>(
        &self,
        peer: &P,
        tcp_notes: &mut TcpConnectTaskNotes,
        task_notes: &ServerTaskNotes,
    ) -> Result<TcpStream, TcpConnectError> {
        let peer_addr = peer.peer_addr();
        let bind_ip = match peer_addr {
            SocketAddr::V4(_) => self.config.bind_v4,
            SocketAddr::V6(_) => self.config.bind_v6,
        };
        tcp_notes.bind = bind_ip.map(BindAddr::Ip).unwrap_or_default();
        tcp_notes.next = Some(peer_addr);
        tcp_notes.expire = peer.expire_datetime();
        tcp_notes.egress = Some(peer.egress_info());
        tcp_notes.tries = 1;
        let instant_now = Instant::now();
        let ret = tokio::time::timeout(
            self.config.tcp_connect_timeout,
            self.try_connect_tcp(peer_addr, &tcp_notes.bind),
        )
        .await;
        tcp_notes.duration = instant_now.elapsed();
        match ret {
            Ok(Ok(ups_stream)) => {
                let local_addr = ups_stream
                    .local_addr()
                    .map_err(TcpConnectError::SetupSocketFailed)?;
                tcp_notes.local = Some(local_addr);
                Ok(ups_stream)
            }
            Ok(Err(e)) => {
                EscapeLogForTcpConnect {
                    tcp_notes,
                    task_id: &task_notes.id,
                }
                .log(&self.escape_logger, &e);
                Err(e)
            }
            Err(_) => {
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

    pub(super) async fn tcp_new_connection<P: NextProxyPeer>(
        &self,
        peer: &P,
        tcp_notes: &mut TcpConnectTaskNotes,
        task_notes: &ServerTaskNotes,
    ) -> Result<LimitedStream<TcpStream>, TcpConnectError> {
        let stream = self.tcp_connect_to(peer, tcp_notes, task_notes).await?;

        let limit_config = peer.tcp_sock_speed_limit();
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
