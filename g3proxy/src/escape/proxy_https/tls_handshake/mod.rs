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

use g3_openssl::{SslConnector, SslInfoCallbackWhere, SslStream};
use g3_types::net::{TlsAlert, TlsAlertType};

use super::ProxyHttpsEscaper;
use crate::log::escape::tls_handshake::{EscapeLogForTlsHandshake, TlsApplication};
use crate::module::tcp_connect::{TcpConnectError, TcpConnectTaskConf, TcpConnectTaskNotes};
use crate::serve::ServerTaskNotes;

impl ProxyHttpsEscaper {
    pub(super) async fn tls_handshake_to_remote(
        &self,
        task_conf: &TcpConnectTaskConf<'_>,
        tcp_notes: &mut TcpConnectTaskNotes,
        task_notes: &ServerTaskNotes,
    ) -> Result<SslStream<impl AsyncRead + AsyncWrite + use<>>, TcpConnectError> {
        let (peer, ups_s) = self
            .tcp_new_connection(task_conf, tcp_notes, task_notes)
            .await?;

        let tls_name = self.config.tls_name.as_ref().unwrap_or_else(|| peer.host());
        let mut ssl = self
            .tls_config
            .build_ssl(tls_name, peer.port())
            .map_err(TcpConnectError::InternalTlsClientError)?;
        let escaper_stats = self.stats.clone();
        ssl.set_info_callback(move |_ssl, r#where, ret| {
            let mask = SslInfoCallbackWhere::from_bits_retain(r#where);
            if !(mask & (SslInfoCallbackWhere::ALERT | SslInfoCallbackWhere::READ)).is_empty() {
                match TlsAlert::new(ret).r#type() {
                    TlsAlertType::Closure => escaper_stats.tls.add_peer_orderly_closure(),
                    TlsAlertType::Error => escaper_stats.tls.add_peer_abortive_closure(),
                }
                escaper_stats.tls.add_handshake_error();
            }
        });

        let connector = SslConnector::new(ssl, ups_s)
            .map_err(|e| TcpConnectError::InternalTlsClientError(anyhow::Error::new(e)))?;

        match tokio::time::timeout(self.tls_config.handshake_timeout, connector.connect()).await {
            Ok(Ok(stream)) => {
                self.stats.tls.add_handshake_success();
                Ok(stream)
            }
            Ok(Err(e)) => {
                self.stats.tls.add_handshake_error();
                let e = anyhow::Error::new(e);
                EscapeLogForTlsHandshake {
                    upstream: task_conf.upstream,
                    tcp_notes,
                    task_id: &task_notes.id,
                    tls_name,
                    tls_peer: &peer,
                    tls_application: TlsApplication::HttpProxy,
                }
                .log(&self.escape_logger, &e);
                Err(TcpConnectError::PeerTlsHandshakeFailed(e))
            }
            Err(_) => {
                self.stats.tls.add_handshake_timeout();
                let e = anyhow!("peer tls handshake timed out");
                EscapeLogForTlsHandshake {
                    upstream: task_conf.upstream,
                    tcp_notes,
                    task_id: &task_notes.id,
                    tls_name,
                    tls_peer: &peer,
                    tls_application: TlsApplication::HttpProxy,
                }
                .log(&self.escape_logger, &e);
                Err(TcpConnectError::PeerTlsHandshakeTimeout)
            }
        }
    }
}
