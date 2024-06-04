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
use openssl::ssl::Ssl;

use g3_dpi::{Protocol, ProtocolInspector};
use g3_io_ext::AggregatedIo;
use g3_openssl::{SslConnector, SslLazyAcceptor};
use g3_types::net::{AlpnProtocol, Host, TlsCertUsage, TlsServiceType};

use super::{TlsInterceptIo, TlsInterceptObject, TlsInterceptionError};
use crate::config::server::ServerConfig;
use crate::inspect::{InterceptionError, StreamInspection};
use crate::serve::ServerTaskResult;

impl<SC> TlsInterceptObject<SC>
where
    SC: ServerConfig + Send + Sync + 'static,
{
    pub(crate) async fn intercept_tlcp(
        mut self,
        inspector: &mut ProtocolInspector,
    ) -> ServerTaskResult<StreamInspection<SC>> {
        match self.do_intercept_tlcp(inspector).await {
            Ok(obj) => {
                self.log_ok();
                Ok(obj)
            }
            Err(e) => {
                self.log_err(&e);
                Err(InterceptionError::Tls(e).into_server_task_error(Protocol::TlsTlcp))
            }
        }
    }

    async fn do_intercept_tlcp(
        &mut self,
        inspector: &mut ProtocolInspector,
    ) -> Result<StreamInspection<SC>, TlsInterceptionError> {
        let TlsInterceptIo {
            clt_r,
            clt_w,
            ups_r,
            ups_w,
        } = self.io.take().unwrap();

        let ssl = Ssl::new(&self.tls_interception.server_config.tlcp_context).map_err(|e| {
            TlsInterceptionError::InternalOpensslServerError(anyhow!(
                "failed to get new TLCP SSL state: {e}"
            ))
        })?;
        let mut lazy_acceptor =
            SslLazyAcceptor::new(ssl, tokio::io::join(clt_r, clt_w)).map_err(|e| {
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
            .fetch_server_name(lazy_acceptor.ssl());
        if let Some(domain) = sni_hostname {
            // TODO also fetch user-site config here?
            self.upstream.set_host(Host::from(domain));
        }
        let alpn_ext = self
            .tls_interception
            .server_config
            .fetch_alpn_extension(lazy_acceptor.ssl())
            .map(|ext| {
                let new_ext = ext.retain_clone(|p| self.retain_alpn_protocol(p));
                if new_ext.is_empty() {
                    // don't block traffic here, return error at the application layer
                    ext.clone()
                } else {
                    // make sure there are still at least 1 client accepted protocol
                    new_ext
                }
            });
        let ups_ssl = match self.ctx.user_site_tls_client() {
            Some(c) => c
                .build_mimic_ssl(sni_hostname, &self.upstream, alpn_ext.as_ref())
                .map_err(|e| {
                    TlsInterceptionError::UpstreamPrepareFailed(anyhow!(
                        "failed to build user-site ssl context: {e}"
                    ))
                })?,
            None => self
                .tls_interception
                .client_config
                .build_tlcp(sni_hostname, &self.upstream, alpn_ext.as_ref())
                .map_err(|e| {
                    TlsInterceptionError::UpstreamPrepareFailed(anyhow!(
                        "failed to build general ssl context: {e}"
                    ))
                })?,
        };

        // fetch fake server cert early in the background
        let cert_domain = sni_hostname
            .map(|v| v.to_string())
            .unwrap_or_else(|| self.upstream.host().to_string());
        let cert_domain: Arc<str> = Arc::from(cert_domain);
        let cert_domain2 = cert_domain.clone();
        let cert_agent = self.tls_interception.cert_agent.clone();
        let sign_pre_fetch_handle = tokio::spawn(async move {
            cert_agent
                .pre_fetch(
                    TlsServiceType::Http,
                    TlsCertUsage::TlcpServerSignature,
                    cert_domain2,
                )
                .await
        });
        let cert_domain2 = cert_domain.clone();
        let cert_agent = self.tls_interception.cert_agent.clone();
        let enc_pre_fetch_handle = tokio::spawn(async move {
            cert_agent
                .pre_fetch(
                    TlsServiceType::Http,
                    TlsCertUsage::TlcpServerEncryption,
                    cert_domain2,
                )
                .await
        });

        // handshake with upstream server
        let ups_tls_connector =
            SslConnector::new(ups_ssl, tokio::io::join(ups_r, ups_w)).map_err(|e| {
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

        let sign_pre_fetch_pair = sign_pre_fetch_handle.await.map_err(|e| {
            TlsInterceptionError::NoFakeCertGenerated(anyhow!(
                "join client cert handle failed: {e}"
            ))
        })?;
        let sign_cert_pair = match sign_pre_fetch_pair {
            Some(pair) => pair,
            None => {
                let upstream_cert = ups_tls_stream.ssl().peer_certificate().ok_or_else(|| {
                    TlsInterceptionError::NoFakeCertGenerated(anyhow!(
                        "failed to get upstream sign certificate"
                    ))
                })?;
                self.tls_interception
                    .cert_agent
                    .fetch(
                        TlsServiceType::Http,
                        TlsCertUsage::TlcpServerSignature,
                        cert_domain.clone(),
                        upstream_cert,
                    )
                    .await
                    .ok_or_else(|| {
                        TlsInterceptionError::NoFakeCertGenerated(anyhow!(
                            "failed to get fake upstream sign certificate"
                        ))
                    })?
            }
        };

        let enc_pre_fetch_pair = enc_pre_fetch_handle.await.map_err(|e| {
            TlsInterceptionError::NoFakeCertGenerated(anyhow!(
                "join client cert handle failed: {e}"
            ))
        })?;
        let enc_cert_pair = match enc_pre_fetch_pair {
            Some(pair) => pair,
            None => {
                let upstream_cert = ups_tls_stream.ssl().peer_certificate().ok_or_else(|| {
                    TlsInterceptionError::NoFakeCertGenerated(anyhow!(
                        "failed to get upstream enc certificate"
                    ))
                })?;
                self.tls_interception
                    .cert_agent
                    .fetch(
                        TlsServiceType::Http,
                        TlsCertUsage::TlcpServerEncryption,
                        cert_domain,
                        upstream_cert,
                    )
                    .await
                    .ok_or_else(|| {
                        TlsInterceptionError::NoFakeCertGenerated(anyhow!(
                            "failed to get fake upstream enc certificate"
                        ))
                    })?
            }
        };

        // set certificate and private key
        let clt_ssl = lazy_acceptor.ssl_mut();
        sign_cert_pair
            .add_sign_to_tlcp(clt_ssl)
            .map_err(TlsInterceptionError::InternalOpensslServerError)?;
        enc_cert_pair
            .add_enc_to_tlcp(clt_ssl)
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
