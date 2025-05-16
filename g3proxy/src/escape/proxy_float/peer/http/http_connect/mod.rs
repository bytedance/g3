/*
 * SPDX-License-Identifier: Apache-2.0
 * Copyright 2023-2025 ByteDance and/or its affiliates.
 */

use std::sync::Arc;

use tokio::io::{AsyncRead, AsyncWrite};
use tokio::net::TcpStream;

use g3_daemon::stat::remote::ArcTcpConnectionTaskRemoteStats;
use g3_http::connect::{HttpConnectRequest, HttpConnectResponse};
use g3_io_ext::{AsyncStream, FlexBufReader, LimitedStream, OnceBufReader};
use g3_openssl::SslStream;

use super::{ProxyFloatEscaper, ProxyFloatHttpPeer};
use crate::log::escape::tls_handshake::TlsApplication;
use crate::module::tcp_connect::{
    TcpConnectError, TcpConnectRemoteWrapperStats, TcpConnectResult, TcpConnectTaskConf,
    TcpConnectTaskNotes, TlsConnectTaskConf,
};
use crate::serve::ServerTaskNotes;

impl ProxyFloatHttpPeer {
    async fn http_connect_tcp_connect_to(
        &self,
        escaper: &ProxyFloatEscaper,
        task_conf: &TcpConnectTaskConf<'_>,
        tcp_notes: &mut TcpConnectTaskNotes,
        task_notes: &ServerTaskNotes,
    ) -> Result<FlexBufReader<LimitedStream<TcpStream>>, TcpConnectError> {
        let mut stream = escaper
            .tcp_new_connection(self, task_conf, tcp_notes, task_notes)
            .await?;

        let req =
            HttpConnectRequest::new(task_conf.upstream, &self.shared_config.append_http_headers);
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
        task_conf: &TcpConnectTaskConf<'_>,
        tcp_notes: &mut TcpConnectTaskNotes,
        task_notes: &ServerTaskNotes,
    ) -> Result<FlexBufReader<LimitedStream<TcpStream>>, TcpConnectError> {
        tokio::time::timeout(
            escaper.config.peer_negotiation_timeout,
            self.http_connect_tcp_connect_to(escaper, task_conf, tcp_notes, task_notes),
        )
        .await
        .map_err(|_| TcpConnectError::NegotiationPeerTimeout)?
    }

    pub(super) async fn http_connect_new_tcp_connection(
        &self,
        escaper: &ProxyFloatEscaper,
        task_conf: &TcpConnectTaskConf<'_>,
        tcp_notes: &mut TcpConnectTaskNotes,
        task_notes: &ServerTaskNotes,
        task_stats: ArcTcpConnectionTaskRemoteStats,
    ) -> TcpConnectResult {
        let mut buf_stream = self
            .timed_http_connect_tcp_connect_to(escaper, task_conf, tcp_notes, task_notes)
            .await?;

        // add in read buffered data
        let r_buffer_size = buf_stream.buffer().len() as u64;
        task_stats.add_read_bytes(r_buffer_size);
        let mut wrapper_stats =
            TcpConnectRemoteWrapperStats::new(escaper.stats.clone(), task_stats);
        let user_stats = escaper.fetch_user_upstream_io_stats(task_notes);
        for s in &user_stats {
            s.io.tcp.add_in_bytes(r_buffer_size);
        }
        wrapper_stats.push_user_io_stats(user_stats);
        let wrapper_stats = Arc::new(wrapper_stats);

        // reset underlying io stats
        buf_stream.get_mut().reset_stats(wrapper_stats.clone());

        let (r, w) = buf_stream.into_split();
        let r = OnceBufReader::from(r);
        Ok((Box::new(r), Box::new(w)))
    }

    pub(super) async fn http_connect_tls_connect_to(
        &self,
        escaper: &ProxyFloatEscaper,
        task_conf: &TlsConnectTaskConf<'_>,
        tcp_notes: &mut TcpConnectTaskNotes,
        task_notes: &ServerTaskNotes,
        tls_application: TlsApplication,
    ) -> Result<SslStream<impl AsyncRead + AsyncWrite + use<>>, TcpConnectError> {
        let buf_stream = self
            .timed_http_connect_tcp_connect_to(escaper, &task_conf.tcp, tcp_notes, task_notes)
            .await?;

        escaper
            .tls_connect_over_tunnel(
                buf_stream.into_inner(),
                task_conf,
                tcp_notes,
                task_notes,
                tls_application,
            )
            .await
    }

    pub(super) async fn http_connect_new_tls_connection(
        &self,
        escaper: &ProxyFloatEscaper,
        task_conf: &TlsConnectTaskConf<'_>,
        tcp_notes: &mut TcpConnectTaskNotes,
        task_notes: &ServerTaskNotes,
        task_stats: ArcTcpConnectionTaskRemoteStats,
    ) -> TcpConnectResult {
        let buf_stream = self
            .timed_http_connect_tcp_connect_to(escaper, &task_conf.tcp, tcp_notes, task_notes)
            .await?;

        escaper
            .new_tls_connection_over_tunnel(
                buf_stream.into_inner(),
                task_conf,
                tcp_notes,
                task_notes,
                task_stats,
            )
            .await
    }
}
