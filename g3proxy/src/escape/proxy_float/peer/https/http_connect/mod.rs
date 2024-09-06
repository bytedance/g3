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

use std::sync::Arc;

use tokio::io::{AsyncRead, AsyncWrite};

use g3_daemon::stat::remote::{
    ArcTcpConnectionTaskRemoteStats, TcpConnectionTaskRemoteStatsWrapper,
};
use g3_http::connect::{HttpConnectRequest, HttpConnectResponse};
use g3_io_ext::{AsyncStream, FlexBufReader, LimitedReader, LimitedWriter, OnceBufReader};
use g3_openssl::SslStream;
use g3_types::net::{Host, OpensslClientConfig};

use super::{ProxyFloatEscaper, ProxyFloatHttpsPeer};
use crate::log::escape::tls_handshake::TlsApplication;
use crate::module::tcp_connect::{TcpConnectError, TcpConnectResult, TcpConnectTaskNotes};
use crate::serve::ServerTaskNotes;

impl ProxyFloatHttpsPeer {
    async fn http_connect_tcp_connect_to(
        &self,
        escaper: &ProxyFloatEscaper,
        tcp_notes: &mut TcpConnectTaskNotes,
        task_notes: &ServerTaskNotes,
    ) -> Result<FlexBufReader<SslStream<impl AsyncRead + AsyncWrite>>, TcpConnectError> {
        let mut stream = escaper
            .tls_handshake_with_peer(tcp_notes, task_notes, &self.tls_name, self)
            .await?;

        let req =
            HttpConnectRequest::new(&tcp_notes.upstream, &self.shared_config.append_http_headers);
        req.send(&mut stream)
            .await
            .map_err(TcpConnectError::NegotiationWriteFailed)?;

        let mut buf_stream = FlexBufReader::new(stream);
        let _ =
            HttpConnectResponse::recv(&mut buf_stream, self.http_connect_rsp_hdr_max_size).await?;

        // TODO detect and set outgoing_addr and target_addr for supported remote proxies
        // set with the registered public ip by default

        Ok(buf_stream)
    }

    async fn timed_http_connect_tcp_connect_to(
        &self,
        escaper: &ProxyFloatEscaper,
        tcp_notes: &mut TcpConnectTaskNotes,
        task_notes: &ServerTaskNotes,
    ) -> Result<FlexBufReader<SslStream<impl AsyncRead + AsyncWrite>>, TcpConnectError> {
        tokio::time::timeout(
            escaper.config.peer_negotiation_timeout,
            self.http_connect_tcp_connect_to(escaper, tcp_notes, task_notes),
        )
        .await
        .map_err(|_| TcpConnectError::NegotiationPeerTimeout)?
    }

    pub(super) async fn http_connect_new_tcp_connection(
        &self,
        escaper: &ProxyFloatEscaper,
        tcp_notes: &mut TcpConnectTaskNotes,
        task_notes: &ServerTaskNotes,
        task_stats: ArcTcpConnectionTaskRemoteStats,
    ) -> TcpConnectResult {
        let buf_stream = self
            .timed_http_connect_tcp_connect_to(escaper, tcp_notes, task_notes)
            .await?;

        // add task and user stats
        // add in read buffered data
        let r_buffer_size = buf_stream.buffer().len() as u64;
        task_stats.add_read_bytes(r_buffer_size);
        let mut wrapper_stats = TcpConnectionTaskRemoteStatsWrapper::new(task_stats);
        let user_stats = escaper.fetch_user_upstream_io_stats(task_notes);
        for s in &user_stats {
            s.io.tcp.add_in_bytes(r_buffer_size);
        }
        wrapper_stats.push_other_stats(user_stats);
        let wrapper_stats = Arc::new(wrapper_stats);

        let (r, w) = buf_stream.into_split();
        let r = OnceBufReader::from(r);
        let r = LimitedReader::new(r, wrapper_stats.clone());
        let w = LimitedWriter::new(w, wrapper_stats);

        Ok((Box::new(r), Box::new(w)))
    }

    pub(super) async fn http_connect_tls_connect_to(
        &self,
        escaper: &ProxyFloatEscaper,
        tcp_notes: &mut TcpConnectTaskNotes,
        task_notes: &ServerTaskNotes,
        tls_config: &OpensslClientConfig,
        tls_name: &Host,
        tls_application: TlsApplication,
    ) -> Result<SslStream<impl AsyncRead + AsyncWrite>, TcpConnectError> {
        let buf_stream = self
            .timed_http_connect_tcp_connect_to(escaper, tcp_notes, task_notes)
            .await?;

        escaper
            .tls_connect_over_tunnel(
                buf_stream.into_inner(),
                tcp_notes,
                task_notes,
                tls_config,
                tls_name,
                tls_application,
            )
            .await
    }

    pub(super) async fn http_connect_new_tls_connection(
        &self,
        escaper: &ProxyFloatEscaper,
        tcp_notes: &mut TcpConnectTaskNotes,
        task_notes: &ServerTaskNotes,
        task_stats: ArcTcpConnectionTaskRemoteStats,
        tls_config: &OpensslClientConfig,
        tls_name: &Host,
    ) -> TcpConnectResult {
        let buf_stream = self
            .timed_http_connect_tcp_connect_to(escaper, tcp_notes, task_notes)
            .await?;

        escaper
            .new_tls_connection_over_tunnel(
                buf_stream.into_inner(),
                tcp_notes,
                task_notes,
                task_stats,
                tls_config,
                tls_name,
            )
            .await
    }
}
