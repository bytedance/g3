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
use openssl::ssl::Ssl;

use g3_dpi::{Protocol, ProtocolInspector};
use g3_io_ext::AggregatedIo;
use g3_openssl::{SslConnector, SslLazyAcceptor};
use g3_types::net::{AlpnProtocol, Host};

use super::{TlsInterceptIo, TlsInterceptObject, TlsInterceptionError};
use crate::config::server::ServerConfig;
use crate::inspect::{InterceptionError, StreamInspection};
use crate::serve::ServerTaskResult;

impl<SC> TlsInterceptObject<SC>
where
    SC: ServerConfig + Send + Sync + 'static,
{
    pub(crate) async fn intercept_modern(
        mut self,
        inspector: &mut ProtocolInspector,
    ) -> ServerTaskResult<StreamInspection<SC>> {
        match self.do_intercept_modern(inspector).await {
            Ok(obj) => {
                self.log_ok();
                Ok(obj)
            }
            Err(e) => {
                self.log_err(&e);
                Err(InterceptionError::Tls(e).into_server_task_error(Protocol::TlsModern))
            }
        }
    }

    async fn do_intercept_modern(
        &mut self,
        inspector: &mut ProtocolInspector,
    ) -> Result<StreamInspection<SC>, TlsInterceptionError> {
        let TlsInterceptIo {
            clt_r,
            clt_w,
            ups_r,
            ups_w,
        } = self.io.take().unwrap();

        let ssl = Ssl::new(&self.tls_interception.server_config.ssl_context).map_err(|e| {
            TlsInterceptionError::InternalOpensslServerError(anyhow!(
                "failed to get new SSL state: {e}"
            ))
        })?;
        let clt_io = AggregatedIo::new(clt_r, clt_w);
        let mut lazy_acceptor = SslLazyAcceptor::new(ssl, clt_io).map_err(|e| {
            TlsInterceptionError::InternalOpensslServerError(anyhow!(
                "failed to create lazy acceptor: {e}"
            ))
        })?;

        // also use upstream timeout config for client handshake
        let accept_timeout = self.tls_interception.server_config.accept_timeout;

        tokio::time::timeout(accept_timeout, lazy_acceptor.accept())
            .await
            .map_err(|_| TlsInterceptionError::ClientHandshakeTimeout)?
            .map_err(|e| {
                TlsInterceptionError::ClientHandshakeFailed(anyhow!(
                    "read client hello msg failed: {e:?}"
                ))
            })?;

        // build to server ssl context based on client hello
        let sni_hostname = self
            .tls_interception
            .server_config
            .server_name(lazy_acceptor.ssl());
        if let Some(domain) = sni_hostname {
            self.upstream.set_host(Host::from(domain));
        }
        let alpn_ext = self
            .tls_interception
            .server_config
            .alpn_extension(lazy_acceptor.ssl());
        let ups_ssl = self
            .tls_interception
            .client_config
            .build_ssl(sni_hostname, &self.upstream, alpn_ext)
            .map_err(|e| {
                TlsInterceptionError::UpstreamPrepareFailed(anyhow!(
                    "failed to build ssl context: {e}"
                ))
            })?;

        // fetch fake server cert early in the background
        let tls_interception = self.tls_interception.clone();
        let cert_domain = sni_hostname
            .map(|v| v.to_string())
            .unwrap_or_else(|| self.upstream.host().to_string());
        let clt_cert_handle =
            tokio::spawn(async move { tls_interception.cert_agent.fetch(cert_domain).await });

        // handshake with upstream server
        let ups_tls_connector = SslConnector::new(ups_ssl, AggregatedIo::new(ups_r, ups_w))
            .map_err(|e| {
                TlsInterceptionError::UpstreamPrepareFailed(anyhow!(
                    "failed to get ssl stream: {e}"
                ))
            })?;
        let ups_tls_stream = tokio::time::timeout(accept_timeout, ups_tls_connector.connect())
            .await
            .map_err(|_| TlsInterceptionError::UpstreamHandshakeTimeout)?
            .map_err(|e| {
                TlsInterceptionError::UpstreamHandshakeFailed(anyhow!(
                    "upstream handshake error: {e}"
                ))
            })?;

        // fetch fake server cert
        let cert_pair = clt_cert_handle
            .await
            .map_err(|e| {
                TlsInterceptionError::NoFakeCertGenerated(anyhow!(
                    "join client cert handle failed: {e}"
                ))
            })?
            .ok_or_else(|| {
                TlsInterceptionError::NoFakeCertGenerated(anyhow!(
                    "failed to get fake upstream certificate"
                ))
            })?;

        // set certificate and private key
        let clt_ssl = lazy_acceptor.ssl_mut();
        cert_pair
            .add_to_ssl(clt_ssl)
            .map_err(TlsInterceptionError::InternalOpensslServerError)?;
        // set alpn
        if let Some(alpn_protocol) = ups_tls_stream.ssl().selected_alpn_protocol() {
            self.tls_interception
                .server_config
                .set_selected_alpn(clt_ssl, alpn_protocol.to_vec());
        }

        let clt_acceptor = lazy_acceptor.into_acceptor(None).map_err(|e| {
            TlsInterceptionError::InternalOpensslServerError(anyhow!(
                "failed to convert acceptor: {e}"
            ))
        })?;
        let clt_tls_stream = tokio::time::timeout(accept_timeout, clt_acceptor.accept())
            .await
            .map_err(|_| TlsInterceptionError::ClientHandshakeTimeout)?
            .map_err(|e| {
                TlsInterceptionError::ClientHandshakeFailed(anyhow!(
                    "client handshake error: {e:?}"
                ))
            })?;

        let mut protocol = Protocol::Unknown;
        let has_alpn = if let Some(alpn_protocol) = clt_tls_stream.ssl().selected_alpn_protocol() {
            if let Some(p) = AlpnProtocol::from_buf(alpn_protocol) {
                inspector.push_alpn_protocol(p);
                protocol = Protocol::from(p);
            }
            true
        } else {
            false
        };

        Ok(self.transfer_connected(protocol, has_alpn, clt_tls_stream, ups_tls_stream))
    }
}
