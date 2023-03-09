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
use rustls::server::AllowAnyAuthenticatedClient;
use rustls::{Certificate, RootCertStore, ServerConfig};

use super::{MultipleCertResolver, RustlsCertificatePair};
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
    client_auth_certs: Option<Vec<Certificate>>,
    accept_timeout: Duration,
}

impl RustlsServerConfigBuilder {
    pub fn empty() -> Self {
        RustlsServerConfigBuilder {
            cert_pairs: Vec::with_capacity(1),
            client_auth: false,
            client_auth_certs: None,
            accept_timeout: Duration::from_secs(10),
        }
    }

    pub fn check(&self) -> anyhow::Result<()> {
        if self.cert_pairs.is_empty() {
            return Err(anyhow!("no cert pair is set"));
        }

        Ok(())
    }

    pub fn enable_client_auth(&mut self) {
        self.client_auth = true;
    }

    pub fn set_client_auth_certificates(&mut self, certs: Vec<Certificate>) {
        self.client_auth_certs = Some(certs);
    }

    pub fn push_cert_pair(&mut self, cert_pair: RustlsCertificatePair) -> anyhow::Result<()> {
        cert_pair.check()?;
        self.cert_pairs.push(cert_pair);
        Ok(())
    }

    pub fn set_accept_timeout(&mut self, timeout: Duration) {
        self.accept_timeout = timeout;
    }

    pub fn build_with_alpn_protocols(
        &self,
        alpn_protocols: Option<Vec<AlpnProtocol>>,
    ) -> anyhow::Result<RustlsServerConfig> {
        let config_builder = ServerConfig::builder().with_safe_defaults();
        let config_builder = if self.client_auth {
            let mut root_store = RootCertStore::empty();
            if let Some(certs) = &self.client_auth_certs {
                for (i, cert) in certs.iter().enumerate() {
                    root_store.add(cert).map_err(|e| {
                        anyhow!("failed to add cert {i} as root certs for client auth: {e:?}",)
                    })?;
                }
            } else {
                let certs = rustls_native_certs::load_native_certs().map_err(|e| {
                    anyhow!("failed to load local root certs for client auth: {e:?}")
                })?;
                let v = certs.into_iter().map(|c| c.0).collect::<Vec<Vec<u8>>>();
                let (_added, _ignored) = root_store.add_parsable_certificates(v.as_slice());
                // debug!("{} added, {} ignored", added, ignored);
            };
            config_builder.with_client_cert_verifier(AllowAnyAuthenticatedClient::new(root_store))
        } else {
            config_builder.with_no_client_auth()
        };

        let mut config = match self.cert_pairs.len() {
            0 => return Err(anyhow!("no cert pair set")),
            1 => {
                let cert_pair = &self.cert_pairs[0];
                config_builder
                    .with_single_cert(cert_pair.certs.clone(), cert_pair.key.clone())
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
