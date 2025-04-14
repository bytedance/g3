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

use anyhow::anyhow;
use tokio::io::{AsyncRead, AsyncWrite};

use g3_io_ext::LimitedStream;
use g3_openssl::{SslConnector, SslStream};
use g3_types::net::{Host, UpstreamAddr};

use super::ProxyFloatEscaper;
use crate::escape::proxy_float::peer::NextProxyPeer;
use crate::log::escape::tls_handshake::{EscapeLogForTlsHandshake, TlsApplication};
use crate::module::tcp_connect::{TcpConnectError, TcpConnectTaskConf, TcpConnectTaskNotes};
use crate::serve::ServerTaskNotes;

impl ProxyFloatEscaper {
    pub(super) async fn tls_handshake_with_peer<P: NextProxyPeer>(
        &self,
        task_conf: &TcpConnectTaskConf<'_>,
        tcp_notes: &mut TcpConnectTaskNotes,
        task_notes: &ServerTaskNotes,
        tls_name: &Host,
        peer: &P,
    ) -> Result<SslStream<LimitedStream<impl AsyncRead + AsyncWrite + use<P>>>, TcpConnectError>
    {
        let stream = self
            .tcp_new_connection(peer, task_conf, tcp_notes, task_notes)
            .await?;
        let peer_addr = peer.peer_addr();

        let ssl = self
            .tls_config
            .build_ssl(tls_name, peer_addr.port())
            .map_err(TcpConnectError::InternalTlsClientError)?;
        let connector = SslConnector::new(ssl, stream)
            .map_err(|e| TcpConnectError::InternalTlsClientError(anyhow::Error::new(e)))?;

        match tokio::time::timeout(self.tls_config.handshake_timeout, connector.connect()).await {
            Ok(Ok(stream)) => {
                self.stats.tls.add_handshake_success();
                Ok(stream)
            }
            Ok(Err(e)) => {
                self.stats.tls.add_handshake_error();
                let e = anyhow::Error::new(e);
                let tls_peer = UpstreamAddr::from(peer_addr);
                if let Some(logger) = &self.escape_logger {
                    EscapeLogForTlsHandshake {
                        upstream: task_conf.upstream,
                        tcp_notes,
                        task_id: &task_notes.id,
                        tls_name,
                        tls_peer: &tls_peer,
                        tls_application: TlsApplication::HttpProxy,
                    }
                    .log(logger, &e);
                }
                Err(TcpConnectError::PeerTlsHandshakeFailed(e))
            }
            Err(_) => {
                self.stats.tls.add_handshake_timeout();
                let tls_peer = UpstreamAddr::from(peer_addr);
                let e = anyhow!("peer tls handshake timed out");
                if let Some(logger) = &self.escape_logger {
                    EscapeLogForTlsHandshake {
                        upstream: task_conf.upstream,
                        tcp_notes,
                        task_id: &task_notes.id,
                        tls_name,
                        tls_peer: &tls_peer,
                        tls_application: TlsApplication::HttpProxy,
                    }
                    .log(logger, &e);
                }
                Err(TcpConnectError::PeerTlsHandshakeTimeout)
            }
        }
    }
}
