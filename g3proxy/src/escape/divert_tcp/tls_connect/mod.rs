/*
 * SPDX-License-Identifier: Apache-2.0
 * Copyright 2024-2025 ByteDance and/or its affiliates.
 */

use std::sync::Arc;

use anyhow::anyhow;
use tokio::io::{AsyncRead, AsyncWrite};

use g3_daemon::stat::remote::{
    ArcTcpConnectionTaskRemoteStats, TcpConnectionTaskRemoteStatsWrapper,
};
use g3_io_ext::{AsyncStream, LimitedReader, LimitedStream, LimitedWriter};
use g3_openssl::{SslConnector, SslStream};

use super::DivertTcpEscaper;
use crate::log::escape::tls_handshake::{EscapeLogForTlsHandshake, TlsApplication};
use crate::module::tcp_connect::{
    TcpConnectError, TcpConnectResult, TcpConnectTaskNotes, TlsConnectTaskConf,
};
use crate::serve::ServerTaskNotes;

impl DivertTcpEscaper {
    pub(super) async fn tls_connect_to(
        &self,
        task_conf: &TlsConnectTaskConf<'_>,
        tcp_notes: &mut TcpConnectTaskNotes,
        task_notes: &ServerTaskNotes,
        tls_application: TlsApplication,
    ) -> Result<SslStream<impl AsyncRead + AsyncWrite + use<>>, TcpConnectError> {
        let stream = self
            .tcp_connect_to(&task_conf.tcp, tcp_notes, task_notes)
            .await?;

        // set limit config and add escaper stats, do not count in task stats
        let limit_config = &self.config.general.tcp_sock_speed_limit;
        let mut stream = LimitedStream::local_limited(
            stream,
            limit_config.shift_millis,
            limit_config.max_south,
            limit_config.max_north,
            self.stats.clone(),
        );

        self.send_pp2_header(
            &mut stream,
            &task_conf.tcp,
            task_notes,
            Some(task_conf.tls_name),
        )
        .await?;

        let ssl = task_conf.build_ssl()?;
        let connector = SslConnector::new(ssl, stream)
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

    pub(super) async fn tls_new_connection(
        &self,
        task_conf: &TlsConnectTaskConf<'_>,
        tcp_notes: &mut TcpConnectTaskNotes,
        task_notes: &ServerTaskNotes,
        task_stats: ArcTcpConnectionTaskRemoteStats,
    ) -> TcpConnectResult {
        let tls_stream = self
            .tls_connect_to(task_conf, tcp_notes, task_notes, TlsApplication::TcpStream)
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
