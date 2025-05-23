/*
 * SPDX-License-Identifier: Apache-2.0
 * Copyright 2023-2025 ByteDance and/or its affiliates.
 */

use std::sync::Arc;
use std::time::Duration;

use anyhow::anyhow;
#[cfg(feature = "quinn")]
use quinn::crypto::rustls::QuicClientConfig;
use rustls::client::Resumption;
use rustls::{ClientConfig, RootCertStore};
use rustls_pki_types::CertificateDer;

use super::RustlsCertificatePair;
use crate::net::tls::AlpnProtocol;

const MINIMAL_HANDSHAKE_TIMEOUT: Duration = Duration::from_millis(100);
const DEFAULT_HANDSHAKE_TIMEOUT: Duration = Duration::from_secs(10);

#[derive(Clone)]
pub struct RustlsClientConfig {
    pub driver: Arc<ClientConfig>,
    pub handshake_timeout: Duration,
}

#[cfg(feature = "quinn")]
#[derive(Clone)]
pub struct RustlsQuicClientConfig {
    pub driver: Arc<QuicClientConfig>,
    pub handshake_timeout: Duration,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct RustlsClientConfigBuilder {
    no_session_cache: bool,
    disable_sni: bool,
    max_fragment_size: Option<usize>,
    client_cert_pair: Option<RustlsCertificatePair>,
    ca_certs: Vec<CertificateDer<'static>>,
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

    pub fn set_ca_certificates(&mut self, certs: Vec<CertificateDer<'static>>) {
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

    fn build_client_config(
        &self,
        alpn_protocols: Option<Vec<AlpnProtocol>>,
    ) -> anyhow::Result<ClientConfig> {
        let config_builder = ClientConfig::builder();

        let mut root_store = RootCertStore::empty();
        if !self.no_default_ca_certs {
            if self.use_builtin_ca_certs {
                root_store.extend(webpki_roots::TLS_SERVER_ROOTS.iter().cloned());
            } else {
                let certs = super::load_native_certs_for_rustls()?;
                for (i, cert) in certs.into_iter().enumerate() {
                    root_store.add(cert).map_err(|e| {
                        anyhow!(
                            "failed to add openssl ca cert {i} as root certs for client auth: {e:?}"
                        )
                    })?;
                }
            }
        }
        for (i, cert) in self.ca_certs.iter().enumerate() {
            root_store.add(cert.clone()).map_err(|e| {
                anyhow!("failed to add cert {i} as root certs for server auth: {e:?}")
            })?;
        }

        let config_builder = config_builder.with_root_certificates(root_store);

        let mut config = if let Some(pair) = &self.client_cert_pair {
            config_builder
                .with_client_auth_cert(pair.certs_owned(), pair.key_owned())
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

        Ok(config)
    }

    pub fn build_with_alpn_protocols(
        &self,
        alpn_protocols: Option<Vec<AlpnProtocol>>,
    ) -> anyhow::Result<RustlsClientConfig> {
        let config = self.build_client_config(alpn_protocols)?;
        Ok(RustlsClientConfig {
            driver: Arc::new(config),
            handshake_timeout: self.handshake_timeout,
        })
    }

    pub fn build(&self) -> anyhow::Result<RustlsClientConfig> {
        self.build_with_alpn_protocols(None)
    }

    #[cfg(feature = "quinn")]
    pub fn build_quic_with_alpn_protocols(
        &self,
        alpn_protocols: Option<Vec<AlpnProtocol>>,
    ) -> anyhow::Result<RustlsQuicClientConfig> {
        let config = self.build_client_config(alpn_protocols)?;
        let quic_config = QuicClientConfig::try_from(config)
            .map_err(|e| anyhow!("invalid quic tls config: {e}"))?;
        Ok(RustlsQuicClientConfig {
            driver: Arc::new(quic_config),
            handshake_timeout: self.handshake_timeout,
        })
    }

    #[cfg(feature = "quinn")]
    pub fn build_quic(&self) -> anyhow::Result<RustlsQuicClientConfig> {
        self.build_quic_with_alpn_protocols(None)
    }
}
