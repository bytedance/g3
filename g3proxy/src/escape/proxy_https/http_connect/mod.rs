/*
 * SPDX-License-Identifier: Apache-2.0
 * Copyright 2023-2025 ByteDance and/or its affiliates.
 */

use std::sync::Arc;

use anyhow::anyhow;
use tokio::io::{AsyncRead, AsyncWrite};

use g3_daemon::stat::remote::{
    ArcTcpConnectionTaskRemoteStats, TcpConnectionTaskRemoteStatsWrapper,
};
use g3_http::connect::{HttpConnectRequest, HttpConnectResponse};
use g3_io_ext::{AsyncStream, FlexBufReader, LimitedReader, LimitedWriter, OnceBufReader};
use g3_openssl::{SslConnector, SslStream};

use super::ProxyHttpsEscaper;
use crate::log::escape::tls_handshake::{EscapeLogForTlsHandshake, TlsApplication};
use crate::module::tcp_connect::{
    TcpConnectError, TcpConnectResult, TcpConnectTaskConf, TcpConnectTaskNotes, TlsConnectTaskConf,
};
use crate::serve::ServerTaskNotes;

impl ProxyHttpsEscaper {
    async fn http_connect_tcp_connect_to(
        &self,
        task_conf: &TcpConnectTaskConf<'_>,
        tcp_notes: &mut TcpConnectTaskNotes,
        task_notes: &ServerTaskNotes,
    ) -> Result<FlexBufReader<SslStream<impl AsyncRead + AsyncWrite + use<>>>, TcpConnectError>
    {
        let mut stream = self
            .tls_handshake_to_remote(task_conf, tcp_notes, task_notes)
            .await?;

        let mut req = HttpConnectRequest::new(task_conf.upstream, &self.config.append_http_headers);

        if self.config.pass_proxy_userid
            && let Some(name) = task_notes.raw_user_name()
        {
            let line = crate::module::http_header::proxy_authorization_basic_pass(name);
            req.append_dyn_header(line);
        }

        req.send(&mut stream)
            .await
            .map_err(TcpConnectError::NegotiationWriteFailed)?;

        let mut buf_stream = FlexBufReader::new(stream);
        let _ =
            HttpConnectResponse::recv(&mut buf_stream, self.config.http_connect_rsp_hdr_max_size)
                .await?;

        // TODO detect and set outgoing_addr and target_addr for supported remote proxies

        Ok(buf_stream)
    }

    async fn timed_http_connect_tcp_connect_to(
        &self,
        task_conf: &TcpConnectTaskConf<'_>,
        tcp_notes: &mut TcpConnectTaskNotes,
        task_notes: &ServerTaskNotes,
    ) -> Result<FlexBufReader<SslStream<impl AsyncRead + AsyncWrite + use<>>>, TcpConnectError>
    {
        tokio::time::timeout(
            self.config.peer_negotiation_timeout,
            self.http_connect_tcp_connect_to(task_conf, tcp_notes, task_notes),
        )
        .await
        .map_err(|_| TcpConnectError::NegotiationPeerTimeout)?
    }

    pub(super) async fn http_connect_new_tcp_connection(
        &self,
        task_conf: &TcpConnectTaskConf<'_>,
        tcp_notes: &mut TcpConnectTaskNotes,
        task_notes: &ServerTaskNotes,
        task_stats: ArcTcpConnectionTaskRemoteStats,
    ) -> TcpConnectResult {
        let buf_stream = self
            .timed_http_connect_tcp_connect_to(task_conf, tcp_notes, task_notes)
            .await?;

        // add task and user stats
        // add in read buffered data
        let r_buffer_size = buf_stream.buffer().len() as u64;
        task_stats.add_read_bytes(r_buffer_size);
        let mut wrapper_stats = TcpConnectionTaskRemoteStatsWrapper::new(task_stats);
        let user_stats = self.fetch_user_upstream_io_stats(task_notes);
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
        task_conf: &TlsConnectTaskConf<'_>,
        tcp_notes: &mut TcpConnectTaskNotes,
        task_notes: &ServerTaskNotes,
        tls_application: TlsApplication,
    ) -> Result<SslStream<impl AsyncRead + AsyncWrite + use<>>, TcpConnectError> {
        let buf_stream = self
            .timed_http_connect_tcp_connect_to(&task_conf.tcp, tcp_notes, task_notes)
            .await?;

        let ssl = task_conf.build_ssl()?;
        let connector = SslConnector::new(ssl, buf_stream.into_inner())
            .map_err(|e| TcpConnectError::InternalTlsClientError(anyhow::Error::new(e)))?;

        match tokio::time::timeout(task_conf.handshake_timeout(), connector.connect()).await {
            Ok(Ok(stream)) => Ok(stream),
            Ok(Err(e)) => {
                let e = anyhow::Error::new(e);
                if let Some(logger) = &self.escape_logger {
                    EscapeLogForTlsHandshake {
                        upstream: task_conf.tcp.upstream,
                        tcp_notes,
                        task_id: &task_notes.id,
                        tls_name: task_conf.tls_name,
                        tls_peer: task_conf.tcp.upstream,
                        tls_application,
                    }
                    .log(logger, &e);
                }
                Err(TcpConnectError::UpstreamTlsHandshakeFailed(e))
            }
            Err(_) => {
                let e = anyhow!("upstream tls handshake timed out");
                if let Some(logger) = &self.escape_logger {
                    EscapeLogForTlsHandshake {
                        upstream: task_conf.tcp.upstream,
                        tcp_notes,
                        task_id: &task_notes.id,
                        tls_name: task_conf.tls_name,
                        tls_peer: task_conf.tcp.upstream,
                        tls_application,
                    }
                    .log(logger, &e);
                }
                Err(TcpConnectError::UpstreamTlsHandshakeTimeout)
            }
        }
    }

    pub(super) async fn http_connect_new_tls_connection(
        &self,
        task_conf: &TlsConnectTaskConf<'_>,
        tcp_notes: &mut TcpConnectTaskNotes,
        task_notes: &ServerTaskNotes,
        task_stats: ArcTcpConnectionTaskRemoteStats,
    ) -> TcpConnectResult {
        let tls_stream = self
            .http_connect_tls_connect_to(
                task_conf,
                tcp_notes,
                task_notes,
                TlsApplication::TcpStream,
            )
            .await?;

        let (ups_r, ups_w) = tls_stream.into_split();

        // add task and user stats
        let mut wrapper_stats = TcpConnectionTaskRemoteStatsWrapper::new(task_stats);
        wrapper_stats.push_other_stats(self.fetch_user_upstream_io_stats(task_notes));
        let wrapper_stats = Arc::new(wrapper_stats);

        let ups_r = LimitedReader::new(ups_r, wrapper_stats.clone());
        let ups_w = LimitedWriter::new(ups_w, wrapper_stats);

        Ok((Box::new(ups_r), Box::new(ups_w)))
    }
}
