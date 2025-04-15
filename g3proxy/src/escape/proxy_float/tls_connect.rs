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

use std::sync::Arc;

use anyhow::anyhow;
use tokio::io::{AsyncRead, AsyncWrite};

use g3_daemon::stat::remote::{
    ArcTcpConnectionTaskRemoteStats, TcpConnectionTaskRemoteStatsWrapper,
};
use g3_io_ext::{AsyncStream, LimitedReader, LimitedWriter};
use g3_openssl::{SslConnector, SslStream};

use super::ProxyFloatEscaper;
use crate::log::escape::tls_handshake::{EscapeLogForTlsHandshake, TlsApplication};
use crate::module::tcp_connect::{
    TcpConnectError, TcpConnectResult, TcpConnectTaskNotes, TlsConnectTaskConf,
};
use crate::serve::ServerTaskNotes;

impl ProxyFloatEscaper {
    pub(super) async fn tls_connect_over_tunnel<S>(
        &self,
        stream: S,
        task_conf: &TlsConnectTaskConf<'_>,
        tcp_notes: &mut TcpConnectTaskNotes,
        task_notes: &ServerTaskNotes,
        tls_application: TlsApplication,
    ) -> Result<SslStream<S>, TcpConnectError>
    where
        S: AsyncRead + AsyncWrite + Sync + Send + Unpin + 'static,
    {
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

    pub(super) async fn new_tls_connection_over_tunnel<S>(
        &self,
        stream: S,
        task_conf: &TlsConnectTaskConf<'_>,
        tcp_notes: &mut TcpConnectTaskNotes,
        task_notes: &ServerTaskNotes,
        task_stats: ArcTcpConnectionTaskRemoteStats,
    ) -> TcpConnectResult
    where
        S: AsyncRead + AsyncWrite + Sync + Send + Unpin + 'static,
    {
        let tls_stream = self
            .tls_connect_over_tunnel(
                stream,
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
