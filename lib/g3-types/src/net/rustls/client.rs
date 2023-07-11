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

use anyhow::anyhow;
use rustls::client::Resumption;
use rustls::{Certificate, ClientConfig, OwnedTrustAnchor, RootCertStore};

use super::RustlsCertificatePair;
use crate::net::tls::AlpnProtocol;

const MINIMAL_HANDSHAKE_TIMEOUT: Duration = Duration::from_millis(100);
const DEFAULT_HANDSHAKE_TIMEOUT: Duration = Duration::from_secs(10);

#[derive(Clone)]
pub struct RustlsClientConfig {
    pub driver: Arc<ClientConfig>,
    pub handshake_timeout: Duration,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct RustlsClientConfigBuilder {
    no_session_cache: bool,
    disable_sni: bool,
    max_fragment_size: Option<usize>,
    client_cert_pair: Option<RustlsCertificatePair>,
    ca_certs: Vec<Certificate>,
    no_default_ca_certs: bool,
    use_builtin_ca_certs: bool,
    handshake_timeout: Duration,
}

impl Default for RustlsClientConfigBuilder {
    fn default() -> Self {
        RustlsClientConfigBuilder {
            no_session_cache: false,
            disable_sni: false,
            max_fragment_size: None,
            client_cert_pair: None,
            ca_certs: vec![],
            no_default_ca_certs: false,
            use_builtin_ca_certs: false,
            handshake_timeout: DEFAULT_HANDSHAKE_TIMEOUT,
        }
    }
}

impl RustlsClientConfigBuilder {
    pub fn check(&mut self) -> anyhow::Result<()> {
        if let Some(cert_pair) = &self.client_cert_pair {
            cert_pair.check()?;
        }

        if self.handshake_timeout < MINIMAL_HANDSHAKE_TIMEOUT {
            self.handshake_timeout = MINIMAL_HANDSHAKE_TIMEOUT;
        }

        Ok(())
    }

    pub fn set_no_session_cache(&mut self) {
        self.no_session_cache = true;
    }

    pub fn set_disable_sni(&mut self) {
        self.disable_sni = true;
    }

    pub fn set_max_fragment_size(&mut self, size: usize) {
        self.max_fragment_size = Some(size);
    }

    pub fn set_cert_pair(&mut self, pair: RustlsCertificatePair) -> Option<RustlsCertificatePair> {
        self.client_cert_pair.replace(pair)
    }

    pub fn set_ca_certificates(&mut self, certs: Vec<Certificate>) {
        self.ca_certs = certs;
    }

    pub fn set_negotiation_timeout(&mut self, timeout: Duration) {
        self.handshake_timeout = timeout;
    }

    pub fn set_no_default_ca_certificates(&mut self) {
        self.no_default_ca_certs = true;
    }

    pub fn set_use_builtin_ca_certificates(&mut self) {
        self.use_builtin_ca_certs = true;
    }

    pub fn build_with_alpn_protocols(
        &self,
        alpn_protocols: Option<Vec<AlpnProtocol>>,
    ) -> anyhow::Result<RustlsClientConfig> {
        let config_builder = ClientConfig::builder().with_safe_defaults();

        let mut root_store = RootCertStore::empty();
        if !self.no_default_ca_certs {
            if self.use_builtin_ca_certs {
                root_store.add_server_trust_anchors(webpki_roots::TLS_SERVER_ROOTS.0.iter().map(
                    |ta| {
                        OwnedTrustAnchor::from_subject_spki_name_constraints(
                            ta.subject,
                            ta.spki,
                            ta.name_constraints,
                        )
                    },
                ));
            } else {
                let certs = super::load_native_certs_for_rustls()?;
                for (i, cert) in certs.iter().enumerate() {
                    root_store.add(cert).map_err(|e| {
                        anyhow!("failed to add openssl ca cert {i} as root certs for client auth: {e:?}",)
                    })?;
                }
            }
        }
        for (i, cert) in self.ca_certs.iter().enumerate() {
            root_store.add(cert).map_err(|e| {
                anyhow!("failed to add cert {i} as root certs for server auth: {e:?}",)
            })?;
        }

        let config_builder = config_builder.with_root_certificates(root_store);

        let mut config = if let Some(pair) = &self.client_cert_pair {
            config_builder
                .with_client_auth_cert(pair.certs.clone(), pair.key.clone())
                .map_err(|e| anyhow!("unable to add client auth certificate: {e:?}"))?
        } else {
            config_builder.with_no_client_auth()
        };

        if let Some(protocols) = alpn_protocols {
            for proto in protocols {
                config
                    .alpn_protocols
                    .push(proto.to_identification_sequence());
            }
        }

        config.max_fragment_size = self.max_fragment_size;
        if self.no_session_cache {
            config.resumption = Resumption::disabled();
        }
        if self.disable_sni {
            config.enable_sni = false;
        }

        Ok(RustlsClientConfig {
            driver: Arc::new(config),
            handshake_timeout: self.handshake_timeout,
        })
    }

    pub fn build(&self) -> anyhow::Result<RustlsClientConfig> {
        self.build_with_alpn_protocols(None)
    }
}
