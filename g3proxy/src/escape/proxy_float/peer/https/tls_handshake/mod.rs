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

use anyhow::anyhow;
use tokio::io::{AsyncRead, AsyncWrite};

use g3_openssl::SslConnector;
use g3_types::net::UpstreamAddr;

use super::ProxyFloatEscaper;
use crate::log::escape::tls_handshake::{EscapeLogForTlsHandshake, TlsApplication};
use crate::module::tcp_connect::{TcpConnectError, TcpConnectTaskNotes};
use crate::serve::ServerTaskNotes;

use super::ProxyFloatHttpsPeer;

impl ProxyFloatHttpsPeer {
    pub(super) async fn tls_handshake_with(
        &self,
        escaper: &ProxyFloatEscaper,
        tcp_notes: &mut TcpConnectTaskNotes,
        task_notes: &ServerTaskNotes,
    ) -> Result<impl AsyncRead + AsyncWrite, TcpConnectError> {
        let stream = escaper
            .tcp_new_connection(self, tcp_notes, task_notes)
            .await?;

        let ssl = escaper
            .tls_config
            .build_ssl(&self.tls_name, self.addr.port())
            .map_err(TcpConnectError::InternalTlsClientError)?;
        let connector = SslConnector::new(ssl, stream)
            .map_err(|e| TcpConnectError::InternalTlsClientError(anyhow::Error::new(e)))?;

        match tokio::time::timeout(escaper.tls_config.handshake_timeout, connector.connect()).await
        {
            Ok(Ok(stream)) => Ok(stream),
            Ok(Err(e)) => {
                let e = anyhow::Error::new(e);
                let tls_peer = UpstreamAddr::from_ip_and_port(self.addr.ip(), self.addr.port());
                EscapeLogForTlsHandshake {
                    tcp_notes,
                    task_id: &task_notes.id,
                    tls_name: &self.tls_name,
                    tls_peer: &tls_peer,
                    tls_application: TlsApplication::HttpProxy,
                }
                .log(&escaper.escape_logger, &e);
                Err(TcpConnectError::PeerTlsHandshakeFailed(e))
            }
            Err(_) => {
                let tls_peer = UpstreamAddr::from_ip_and_port(self.addr.ip(), self.addr.port());
                let e = anyhow!("peer tls handshake timed out");
                EscapeLogForTlsHandshake {
                    tcp_notes,
                    task_id: &task_notes.id,
                    tls_name: &self.tls_name,
                    tls_peer: &tls_peer,
                    tls_application: TlsApplication::HttpProxy,
                }
                .log(&escaper.escape_logger, &e);
                Err(TcpConnectError::PeerTlsHandshakeTimeout)
            }
        }
    }
}
