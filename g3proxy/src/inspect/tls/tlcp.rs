/*
 * SPDX-License-Identifier: Apache-2.0
 * Copyright 2024-2025 ByteDance and/or its affiliates.
 */

use anyhow::anyhow;
use arcstr::ArcStr;
use openssl::ssl::Ssl;

use g3_dpi::{Protocol, ProtocolInspector};
use g3_io_ext::OnceBufReader;
use g3_openssl::{SslAcceptor, SslConnector};
use g3_types::net::{AlpnProtocol, Host, TlsCertUsage, TlsServiceType};

use super::{ParsedClientHello, TlsInterceptIo, TlsInterceptObject, TlsInterceptionError};
use crate::config::server::ServerConfig;
use crate::inspect::StreamInspection;

impl<SC> TlsInterceptObject<SC>
where
    SC: ServerConfig + Send + Sync + 'static,
{
    pub(super) async fn do_intercept_tlcp(
        &mut self,
        client_hello: ParsedClientHello,
        inspector: &mut ProtocolInspector,
    ) -> Result<StreamInspection<SC>, TlsInterceptionError> {
        let TlsInterceptIo {
            clt_r_buf,
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
        let alpn_ext = client_hello.alpn.as_ref().map(|ext| {
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
            .map(ArcStr::from)
            .unwrap_or_else(|| self.upstream.host().to_arc_str());
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

        let sign_pre_fetch_pair = sign_pre_fetch_handle.await.map_err(|e| {
            TlsInterceptionError::NoFakeCertGenerated(anyhow!(
                "join client cert handle failed: {e}"
            ))
        })?;
        let sign_cert_pair = match sign_pre_fetch_pair {
            Some(pair) => pair,
            None => self
                .tls_interception
                .cert_agent
                .fetch(
                    TlsServiceType::Http,
                    TlsCertUsage::TlcpServerSignature,
                    cert_domain.clone(),
                    upstream_cert.clone(),
                )
                .await
                .ok_or_else(|| {
                    TlsInterceptionError::NoFakeCertGenerated(anyhow!(
                        "failed to get fake upstream sign certificate"
                    ))
                })?,
        };

        let enc_pre_fetch_pair = enc_pre_fetch_handle.await.map_err(|e| {
            TlsInterceptionError::NoFakeCertGenerated(anyhow!(
                "join client cert handle failed: {e}"
            ))
        })?;
        let enc_cert_pair = match enc_pre_fetch_pair {
            Some(pair) => pair,
            None => self
                .tls_interception
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
                })?,
        };

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

        let mut protocol = Protocol::Unknown;
        let has_alpn = if let Some(alpn_protocol) = clt_tls_stream.ssl().selected_alpn_protocol() {
            if let Some(p) = AlpnProtocol::from_selected(alpn_protocol) {
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
