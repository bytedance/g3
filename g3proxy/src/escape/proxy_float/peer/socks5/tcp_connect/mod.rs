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

use tokio::net::{tcp, TcpStream};
use tokio::time::Instant;

use g3_io_ext::{LimitedReader, LimitedWriter};
use g3_types::net::ConnectError;

use super::ProxyFloatSocks5Peer;
use crate::log::escape::tcp_connect::EscapeLogForTcpConnect;
use crate::module::tcp_connect::{TcpConnectError, TcpConnectTaskNotes};
use crate::serve::ServerTaskNotes;

impl ProxyFloatSocks5Peer {
    async fn try_connect_tcp(
        &self,
        peer: SocketAddr,
        bind: Option<IpAddr>,
    ) -> Result<TcpStream, TcpConnectError> {
        // use new socket every time, as we set bind_no_port
        let sock = g3_socket::tcp::new_socket_to(
            peer.ip(),
            bind,
            &self.escaper_config.tcp_keepalive,
            &self.escaper_config.tcp_misc_opts,
            true,
        )
        .map_err(TcpConnectError::SetupSocketFailed)?;
        self.escaper_stats.tcp.add_connection_attempted();
        match sock.connect(peer).await {
            Ok(ups_stream) => {
                self.escaper_stats.tcp.add_connection_established();
                Ok(ups_stream)
            }
            Err(e) => Err(TcpConnectError::ConnectFailed(ConnectError::from(e))),
        }
    }

    async fn tcp_connect_to<'a>(
        &'a self,
        tcp_notes: &'a mut TcpConnectTaskNotes,
        task_notes: &'a ServerTaskNotes,
    ) -> Result<TcpStream, TcpConnectError> {
        let bind = match self.addr {
            SocketAddr::V4(_) => self.escaper_config.bind_v4,
            SocketAddr::V6(_) => self.escaper_config.bind_v6,
        };
        tcp_notes.bind = bind;
        tcp_notes.next = Some(self.addr);
        tcp_notes.expire = self.shared_config.expire_datetime;
        tcp_notes.egress = Some(self.egress_info.clone());
        tcp_notes.tries = 1;
        let instant_now = Instant::now();
        let ret = tokio::time::timeout(
            self.escaper_config.tcp_connect_timeout,
            self.try_connect_tcp(self.addr, tcp_notes.bind),
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

    pub(super) async fn tcp_new_connection<'a>(
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
        let stream = self.tcp_connect_to(tcp_notes, task_notes).await?;
        let (r, w) = stream.into_split();

        let limit_config = &self.shared_config.tcp_conn_speed_limit;
        let r = LimitedReader::new(
            r,
            limit_config.shift_millis,
            limit_config.max_south,
            self.escaper_stats.for_limited_reader(),
        );
        let w = LimitedWriter::new(
            w,
            limit_config.shift_millis,
            limit_config.max_north,
            self.escaper_stats.for_limited_writer(),
        );

        Ok((r, w))
    }
}
