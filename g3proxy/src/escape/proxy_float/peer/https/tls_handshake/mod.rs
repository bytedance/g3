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

use std::pin::Pin;

use anyhow::anyhow;
use tokio::io::{AsyncRead, AsyncWrite};
use tokio_openssl::SslStream;

use g3_io_ext::AggregatedIo;
use g3_types::net::UpstreamAddr;

use crate::log::escape::tls_handshake::{EscapeLogForTlsHandshake, TlsApplication};
use crate::module::tcp_connect::{TcpConnectError, TcpConnectTaskNotes};
use crate::serve::ServerTaskNotes;

use super::ProxyFloatHttpsPeer;

impl ProxyFloatHttpsPeer {
    pub(super) async fn tls_handshake_with<'a>(
        &'a self,
        tcp_notes: &'a mut TcpConnectTaskNotes,
        task_notes: &'a ServerTaskNotes,
    ) -> Result<(impl AsyncRead, impl AsyncWrite), TcpConnectError> {
        let (r, w) = self.tcp_new_connection(tcp_notes, task_notes).await?;

        let ssl = self
            .tls_config
            .build_ssl(&self.tls_name, self.addr.port())
            .map_err(TcpConnectError::InternalTlsClientError)?;
        let mut stream = SslStream::new(
            ssl,
            AggregatedIo {
                reader: r,
                writer: w,
            },
        )
        .map_err(|e| TcpConnectError::InternalTlsClientError(anyhow::Error::new(e)))?;

        match tokio::time::timeout(
            self.tls_config.handshake_timeout,
            Pin::new(&mut stream).connect(),
        )
        .await
        {
            Ok(Ok(_)) => {
                let (r, w) = tokio::io::split(stream);

                Ok((r, w))
            }
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
                .log(&self.escape_logger, &e);
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
                .log(&self.escape_logger, &e);
                Err(TcpConnectError::PeerTlsHandshakeTimeout)
            }
        }
    }
}
