/*
 * SPDX-License-Identifier: Apache-2.0
 * Copyright 2025 ByteDance and/or its affiliates.
 */

use std::sync::Arc;

use anyhow::anyhow;
use bytes::BytesMut;
use openssl::ssl::Ssl;

use g3_io_ext::OnceBufReader;
use g3_openssl::{SslAcceptor, SslConnector};
use g3_types::net::{Host, TlsCertUsage, TlsServiceType};

use super::{StartTlsInterceptIo, StartTlsInterceptObject};
use crate::config::server::ServerConfig;
use crate::inspect::StreamInspection;
use crate::inspect::tls::{ParsedClientHello, TlsInterceptionError};

impl<SC> StartTlsInterceptObject<SC>
where
    SC: ServerConfig + Send + Sync + 'static,
{
    pub(super) async fn do_intercept_tlcp(
        &mut self,
        client_hello: ParsedClientHello,
        clt_r_buf: BytesMut,
    ) -> Result<StreamInspection<SC>, TlsInterceptionError> {
        let StartTlsInterceptIo {
            clt_r,
            clt_w,
            ups_r,
            ups_w,
        } = self.io.take().unwrap();

        let mut clt_ssl =
            Ssl::new(&self.tls_interception.server_config.tlcp_context).map_err(|e| {
                TlsInterceptionError::InternalOpensslServerError(anyhow!(
                    "failed to get new TLCP SSL state: {e}"
                ))
            })?;

        // build to server ssl context based on client hello
        let sni_hostname = client_hello.sni.as_ref();
        if let Some(domain) = sni_hostname {
            // TODO also fetch user-site config here?
            self.upstream.set_host(Host::from(domain));
        }
        let alpn_ext = client_hello.alpn.as_ref();
        let ups_ssl = match self.ctx.user_site_tls_client() {
            Some(c) => c
                .build_mimic_ssl(sni_hostname, &self.upstream, alpn_ext)
                .map_err(|e| {
                    TlsInterceptionError::UpstreamPrepareFailed(anyhow!(
                        "failed to build user-site ssl context: {e}"
                    ))
                })?,
            None => self
                .tls_interception
                .client_config
                .build_tlcp(sni_hostname, &self.upstream, alpn_ext)
                .map_err(|e| {
                    TlsInterceptionError::UpstreamPrepareFailed(anyhow!(
                        "failed to build general ssl context: {e}"
                    ))
                })?,
        };

        // handshake with upstream server
        let ups_tls_connector =
            SslConnector::new(ups_ssl, tokio::io::join(ups_r, ups_w)).map_err(|e| {
                TlsInterceptionError::UpstreamPrepareFailed(anyhow!(
                    "failed to get ssl stream: {e}"
                ))
            })?;
        let ups_tls_stream = tokio::time::timeout(
            self.tls_interception.client_config.handshake_timeout,
            ups_tls_connector.connect(),
        )
        .await
        .map_err(|_| TlsInterceptionError::UpstreamHandshakeTimeout)?
        .map_err(|e| {
            TlsInterceptionError::UpstreamHandshakeFailed(anyhow!("upstream handshake error: {e}"))
        })?;

        let upstream_cert = ups_tls_stream.ssl().peer_certificate().ok_or_else(|| {
            TlsInterceptionError::NoFakeCertGenerated(anyhow!("failed to get upstream certificate"))
        })?;
        self.server_verify_result = Some(ups_tls_stream.ssl().verify_result());
        let cert_domain = sni_hostname
            .map(|v| v.to_string())
            .unwrap_or_else(|| self.upstream.host().to_string());
        let cert_domain: Arc<str> = Arc::from(cert_domain);

        let tls_service_type = TlsServiceType::from(self.protocol);
        let sign_cert_pair = self
            .tls_interception
            .cert_agent
            .fetch(
                tls_service_type,
                TlsCertUsage::TlcpServerSignature,
                cert_domain.clone(),
                upstream_cert.clone(),
            )
            .await
            .ok_or_else(|| {
                TlsInterceptionError::NoFakeCertGenerated(anyhow!(
                    "failed to get fake upstream sign certificate"
                ))
            })?;
        let enc_cert_pair = self
            .tls_interception
            .cert_agent
            .fetch(
                tls_service_type,
                TlsCertUsage::TlcpServerEncryption,
                cert_domain.clone(),
                upstream_cert,
            )
            .await
            .ok_or_else(|| {
                TlsInterceptionError::NoFakeCertGenerated(anyhow!(
                    "failed to get fake upstream enc certificate"
                ))
            })?;

        // set certificate and private key
        sign_cert_pair
            .add_sign_to_tlcp(&mut clt_ssl)
            .map_err(TlsInterceptionError::InternalOpensslServerError)?;
        enc_cert_pair
            .add_enc_to_tlcp(&mut clt_ssl)
            .map_err(TlsInterceptionError::InternalOpensslServerError)?;
        // set alpn
        if let Some(alpn_protocol) = ups_tls_stream.ssl().selected_alpn_protocol() {
            self.tls_interception
                .server_config
                .set_selected_alpn(&mut clt_ssl, alpn_protocol.to_vec());
        }

        let clt_acceptor = SslAcceptor::new(
            clt_ssl,
            tokio::io::join(OnceBufReader::new(clt_r, clt_r_buf), clt_w),
            self.tls_interception.server_config.accept_timeout,
        )
        .map_err(|e| {
            TlsInterceptionError::InternalOpensslServerError(anyhow!(
                "failed to convert acceptor: {e}"
            ))
        })?;
        let clt_tls_stream = clt_acceptor.accept().await.map_err(|e| {
            TlsInterceptionError::ClientHandshakeFailed(anyhow!("client handshake error: {e:?}"))
        })?;

        Ok(self.transfer_connected(clt_tls_stream, ups_tls_stream))
    }
}
