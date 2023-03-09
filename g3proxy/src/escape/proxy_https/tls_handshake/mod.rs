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

use std::borrow::Cow;
use std::pin::Pin;

use anyhow::anyhow;
use tokio::io::{AsyncRead, AsyncWrite};
use tokio_openssl::SslStream;

use g3_io_ext::AggregatedIo;

use super::ProxyHttpsEscaper;
use crate::log::escape::tls_handshake::{EscapeLogForTlsHandshake, TlsApplication};
use crate::module::tcp_connect::{TcpConnectError, TcpConnectTaskNotes};
use crate::serve::ServerTaskNotes;

impl ProxyHttpsEscaper {
    pub(super) async fn tls_handshake_to_remote<'a>(
        &'a self,
        tcp_notes: &'a mut TcpConnectTaskNotes,
        task_notes: &'a ServerTaskNotes,
    ) -> Result<(impl AsyncRead, impl AsyncWrite), TcpConnectError> {
        let (peer, ups_r, ups_w) = self.tcp_new_connection(tcp_notes, task_notes).await?;

        let tls_name = self
            .config
            .tls_name
            .as_ref()
            .map(|v| Cow::Borrowed(v.as_str()))
            .unwrap_or_else(|| peer.host_str());
        let ssl = self
            .tls_config
            .build_ssl(&tls_name, peer.port())
            .map_err(TcpConnectError::InternalTlsClientError)?;
        let mut stream = SslStream::new(
            ssl,
            AggregatedIo {
                reader: ups_r,
                writer: ups_w,
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
                EscapeLogForTlsHandshake {
                    tcp_notes,
                    task_id: &task_notes.id,
                    tls_name: &tls_name,
                    tls_peer: &peer,
                    tls_application: TlsApplication::HttpProxy,
                }
                .log(&self.escape_logger, &e);
                Err(TcpConnectError::PeerTlsHandshakeFailed(e))
            }
            Err(_) => {
                let e = anyhow!("peer tls handshake timed out");
                EscapeLogForTlsHandshake {
                    tcp_notes,
                    task_id: &task_notes.id,
                    tls_name: &tls_name,
                    tls_peer: &peer,
                    tls_application: TlsApplication::HttpProxy,
                }
                .log(&self.escape_logger, &e);
                Err(TcpConnectError::PeerTlsHandshakeTimeout)
            }
        }
    }
}
