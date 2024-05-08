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

use std::sync::Arc;
use std::time::Duration;

use anyhow::{anyhow, Context};
use rustls::crypto::ring::Ticketer;
use rustls::server::WebPkiClientVerifier;
use rustls::{RootCertStore, ServerConfig};
use rustls_pki_types::CertificateDer;

use super::{MultipleCertResolver, RustlsCertificatePair, RustlsServerSessionCache};
use crate::net::tls::AlpnProtocol;

#[derive(Clone)]
pub struct RustlsServerConfig {
    pub driver: Arc<ServerConfig>,
    pub accept_timeout: Duration,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct RustlsServerConfigBuilder {
    cert_pairs: Vec<RustlsCertificatePair>,
    client_auth: bool,
    client_auth_certs: Option<Vec<CertificateDer<'static>>>,
    use_session_ticket: bool,
    accept_timeout: Duration,
}

impl RustlsServerConfigBuilder {
    pub fn empty() -> Self {
        RustlsServerConfigBuilder {
            cert_pairs: Vec::with_capacity(1),
            client_auth: false,
            client_auth_certs: None,
            use_session_ticket: false,
            accept_timeout: Duration::from_secs(10),
        }
    }

    pub fn check(&self) -> anyhow::Result<()> {
        if self.cert_pairs.is_empty() {
            return Err(anyhow!("no cert pair is set"));
        }

        Ok(())
    }

    pub fn set_use_session_ticket(&mut self, enable: bool) {
        self.use_session_ticket = enable;
    }

    pub fn enable_client_auth(&mut self) {
        self.client_auth = true;
    }

    pub fn set_client_auth_certificates(&mut self, certs: Vec<CertificateDer<'static>>) {
        self.client_auth_certs = Some(certs);
    }

    pub fn push_cert_pair(&mut self, cert_pair: RustlsCertificatePair) {
        self.cert_pairs.push(cert_pair);
    }

    #[inline]
    pub fn set_accept_timeout(&mut self, timeout: Duration) {
        self.accept_timeout = timeout;
    }

    #[inline]
    pub fn accept_timeout(&self) -> Duration {
        self.accept_timeout
    }

    pub fn build_with_alpn_protocols(
        &self,
        alpn_protocols: Option<Vec<AlpnProtocol>>,
    ) -> anyhow::Result<RustlsServerConfig> {
        let config_builder = ServerConfig::builder();
        let config_builder = if self.client_auth {
            let mut root_store = RootCertStore::empty();
            if let Some(certs) = &self.client_auth_certs {
                for (i, cert) in certs.iter().enumerate() {
                    root_store.add(cert.clone()).map_err(|e| {
                        anyhow!("failed to add cert {i} as root certs for client auth: {e:?}",)
                    })?;
                }
            } else {
                let certs = super::load_native_certs_for_rustls()?;
                for (i, cert) in certs.into_iter().enumerate() {
                    root_store.add(cert).map_err(|e| {
                        anyhow!("failed to add openssl ca cert {i} as root certs for client auth: {e:?}",)
                    })?;
                }
            };
            let client_verifier = WebPkiClientVerifier::builder(Arc::new(root_store))
                .build()
                .map_err(|e| anyhow!("failed to build client cert verifier: {e}"))?;
            config_builder.with_client_cert_verifier(client_verifier)
        } else {
            config_builder.with_no_client_auth()
        };

        let mut config = match self.cert_pairs.len() {
            0 => return Err(anyhow!("no cert pair set")),
            1 => {
                let cert_pair = &self.cert_pairs[0];
                config_builder
                    .with_single_cert(cert_pair.certs_owned(), cert_pair.key_owned())
                    .map_err(|e| anyhow!("failed to set server cert pair: {e:?}"))?
            }
            n => {
                let mut cert_resolver = MultipleCertResolver::with_capacity(n);
                for (i, pair) in self.cert_pairs.iter().enumerate() {
                    cert_resolver
                        .push_cert_pair(pair)
                        .context(format!("failed to set server cert pair #{i}"))?;
                }
                config_builder.with_cert_resolver(Arc::new(cert_resolver))
            }
        };
        config.session_storage = Arc::new(RustlsServerSessionCache::default());
        if self.use_session_ticket {
            let ticketer =
                Ticketer::new().map_err(|e| anyhow!("failed to create session ticketer: {e}"))?;
            config.ticketer = ticketer;
        }

        if let Some(protocols) = alpn_protocols {
            for proto in protocols {
                config
                    .alpn_protocols
                    .push(proto.to_identification_sequence());
            }
        }

        Ok(RustlsServerConfig {
            driver: Arc::new(config),
            accept_timeout: self.accept_timeout,
        })
    }

    pub fn build(&self) -> anyhow::Result<RustlsServerConfig> {
        self.build_with_alpn_protocols(None)
    }
}
