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

use std::time::Duration;

use anyhow::anyhow;
use openssl::ssl::{
    Ssl, SslConnector, SslConnectorBuilder, SslContext, SslMethod, SslVerifyMode, SslVersion,
};
use openssl::x509::store::X509StoreBuilder;
use openssl::x509::X509;

use super::{OpensslCertificatePair, OpensslProtocol};
use crate::net::tls::AlpnProtocol;
use crate::net::Host;

#[cfg(feature = "tongsuo")]
use super::OpensslTlcpCertificatePair;

mod intercept;
pub use intercept::{OpensslInterceptionClientConfig, OpensslInterceptionClientConfigBuilder};

mod session;
use session::{OpensslClientSessionCache, OpensslSessionCacheConfig};

const MINIMAL_HANDSHAKE_TIMEOUT: Duration = Duration::from_millis(100);
const DEFAULT_HANDSHAKE_TIMEOUT: Duration = Duration::from_secs(10);

#[derive(Clone)]
pub struct OpensslClientConfig {
    disable_sni: bool,
    ssl_context: SslContext,
    pub handshake_timeout: Duration,
    session_cache: Option<OpensslClientSessionCache>,
}

impl OpensslClientConfig {
    pub fn build_ssl(&self, tls_name: &Host, port: u16) -> anyhow::Result<Ssl> {
        let mut ssl =
            Ssl::new(&self.ssl_context).map_err(|e| anyhow!("failed to get new Ssl state: {e}"))?;
        let verify_param = ssl.param_mut();
        match tls_name {
            Host::Domain(domain) => {
                verify_param
                    .set_host(domain)
                    .map_err(|e| anyhow!("failed to set cert verify domain: {e}"))?;
                if !self.disable_sni {
                    ssl.set_hostname(domain)
                        .map_err(|e| anyhow!("failed to set sni hostname: {e}"))?;
                }
            }
            Host::Ip(ip) => {
                verify_param
                    .set_ip(*ip)
                    .map_err(|e| anyhow!("failed to set cert verify ip: {e}"))?;
            }
        }
        if let Some(cache) = &self.session_cache {
            cache.find_and_set_cache(&mut ssl, tls_name, port)?;
        }
        Ok(ssl)
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct OpensslClientConfigBuilder {
    protocol: Option<OpensslProtocol>,
    ciphers: Vec<String>,
    disable_sni: bool,
    ca_certs: Vec<Vec<u8>>,
    no_default_ca_certs: bool,
    client_cert_pair: Option<OpensslCertificatePair>,
    #[cfg(feature = "tongsuo")]
    client_tlcp_cert_pair: Option<OpensslTlcpCertificatePair>,
    handshake_timeout: Duration,
    session_cache: OpensslSessionCacheConfig,
}

impl Default for OpensslClientConfigBuilder {
    fn default() -> Self {
        OpensslClientConfigBuilder {
            protocol: None,
            ciphers: Vec::new(),
            disable_sni: false,
            ca_certs: Vec::new(),
            no_default_ca_certs: false,
            client_cert_pair: None,
            #[cfg(feature = "tongsuo")]
            client_tlcp_cert_pair: None,
            handshake_timeout: DEFAULT_HANDSHAKE_TIMEOUT,
            session_cache: OpensslSessionCacheConfig::default(),
        }
    }
}

impl OpensslClientConfigBuilder {
    pub fn with_cache_for_one_site() -> Self {
        OpensslClientConfigBuilder {
            session_cache: OpensslSessionCacheConfig::new_for_one(),
            ..Default::default()
        }
    }

    pub fn with_cache_for_many_sites() -> Self {
        OpensslClientConfigBuilder {
            session_cache: OpensslSessionCacheConfig::new_for_many(),
            ..Default::default()
        }
    }

    pub fn check(&mut self) -> anyhow::Result<()> {
        if let Some(cert_pair) = &self.client_cert_pair {
            cert_pair.check()?;
        }

        #[cfg(feature = "tongsuo")]
        if let Some(tlcp_cert_pair) = &self.client_tlcp_cert_pair {
            tlcp_cert_pair.check()?;
        }

        if !self.ciphers.is_empty() && self.protocol.is_none() {
            return Err(anyhow!(
                "protocol should be set to a fixed version if you want to specify cipher list / ciphersuites"
            ));
        }

        if self.handshake_timeout < MINIMAL_HANDSHAKE_TIMEOUT {
            self.handshake_timeout = MINIMAL_HANDSHAKE_TIMEOUT;
        }

        Ok(())
    }

    pub fn set_protocol(&mut self, protocol: OpensslProtocol) {
        self.protocol = Some(protocol);
    }

    pub fn set_ciphers(&mut self, ciphers: Vec<String>) {
        self.ciphers = ciphers;
    }

    pub fn set_disable_sni(&mut self) {
        self.disable_sni = true;
    }

    pub fn set_ca_certificates(&mut self, certs: Vec<X509>) -> anyhow::Result<()> {
        let mut all_der = Vec::with_capacity(certs.len());
        for (i, cert) in certs.into_iter().enumerate() {
            let bytes = cert
                .to_der()
                .map_err(|e| anyhow!("failed to encode ca certificate #{i}: {e}"))?;
            all_der.push(bytes);
        }
        self.ca_certs = all_der;
        Ok(())
    }

    pub fn set_no_default_ca_certificates(&mut self) {
        self.no_default_ca_certs = true;
    }

    pub fn set_handshake_timeout(&mut self, timeout: Duration) {
        self.handshake_timeout = timeout;
    }

    pub fn set_cert_pair(
        &mut self,
        pair: OpensslCertificatePair,
    ) -> Option<OpensslCertificatePair> {
        self.client_cert_pair.replace(pair)
    }

    #[cfg(feature = "tongsuo")]
    pub fn set_tlcp_cert_pair(
        &mut self,
        pair: OpensslTlcpCertificatePair,
    ) -> Option<OpensslTlcpCertificatePair> {
        self.client_tlcp_cert_pair.replace(pair)
    }

    #[inline]
    pub fn set_no_session_cache(&mut self) {
        self.session_cache.set_no_session_cache();
    }

    #[inline]
    pub fn set_use_builtin_session_cache(&mut self) {
        self.session_cache.set_use_builtin_session_cache();
    }

    #[inline]
    pub fn set_session_cache_sites_count(&mut self, max: usize) {
        self.session_cache.set_sites_count(max);
    }

    #[inline]
    pub fn set_session_cache_each_capacity(&mut self, cap: usize) {
        self.session_cache.set_each_capacity(cap);
    }

    #[cfg(feature = "tongsuo")]
    fn new_tlcp_builder(&self) -> anyhow::Result<SslConnectorBuilder> {
        let mut ctx_builder = SslConnector::builder(SslMethod::ntls_client())
            .map_err(|e| anyhow!("failed to create ssl context builder: {e}"))?;
        ctx_builder.set_verify(SslVerifyMode::PEER);
        ctx_builder.enable_ntls();

        let mut use_dhe = false;
        if let Some(cert_pair) = &self.client_tlcp_cert_pair {
            cert_pair.add_to_client_ssl_context(&mut ctx_builder)?;
            use_dhe = true;
        }

        if !self.ciphers.is_empty() {
            let cipher_list = self.ciphers.join(":");
            ctx_builder
                .set_cipher_list(&cipher_list)
                .map_err(|e| anyhow!("failed to set cipher list: {e}"))?;
        } else if use_dhe {
            ctx_builder
                .set_cipher_list(
                    "ECDHE-SM2-SM4-GCM-SM3:ECC-SM2-SM4-GCM-SM3:ECDHE-SM2-SM4-CBC-SM3:ECC-SM2-SM4-CBC-SM3:\
                     RSA-SM4-GCM-SM3:RSA-SM4-GCM-SHA256:RSA-SM4-CBC-SM3:RSA-SM4-CBC-SHA256",
                )
                .map_err(|e| anyhow!("failed to set cipher list: {e}"))?;
        } else {
            ctx_builder
                .set_cipher_list(
                    "ECC-SM2-SM4-GCM-SM3:ECC-SM2-SM4-CBC-SM3:\
                     RSA-SM4-GCM-SM3:RSA-SM4-GCM-SHA256:RSA-SM4-CBC-SM3:RSA-SM4-CBC-SHA256",
                )
                .map_err(|e| anyhow!("failed to set cipher list: {e}"))?;
        }

        Ok(ctx_builder)
    }

    fn new_tls13_builder(&self) -> anyhow::Result<SslConnectorBuilder> {
        let mut ctx_builder = SslConnector::builder(SslMethod::tls_client())
            .map_err(|e| anyhow!("failed to create ssl context builder: {e}"))?;
        ctx_builder.set_verify(SslVerifyMode::PEER);

        ctx_builder
            .set_min_proto_version(Some(SslVersion::TLS1_3))
            .map_err(|e| anyhow!("failed to set min protocol version: {e}"))?;
        ctx_builder
            .set_max_proto_version(Some(SslVersion::TLS1_3))
            .map_err(|e| anyhow!("failed to set max protocol version: {e}"))?;

        if !self.ciphers.is_empty() {
            let ciphersuites = self.ciphers.join(":");
            ctx_builder
                .set_ciphersuites(&ciphersuites)
                .map_err(|e| anyhow!("failed to set ciphersuites: {e}"))?;
        }

        if let Some(cert_pair) = &self.client_cert_pair {
            cert_pair.add_to_client_ssl_context(&mut ctx_builder)?;
        }

        Ok(ctx_builder)
    }

    fn new_versioned_builder(&self, version: SslVersion) -> anyhow::Result<SslConnectorBuilder> {
        let mut ctx_builder = SslConnector::builder(SslMethod::tls_client())
            .map_err(|e| anyhow!("failed to create ssl context builder: {e}"))?;
        ctx_builder.set_verify(SslVerifyMode::PEER);

        ctx_builder
            .set_min_proto_version(Some(version))
            .map_err(|e| anyhow!("failed to set min protocol version: {e}"))?;
        ctx_builder
            .set_max_proto_version(Some(version))
            .map_err(|e| anyhow!("failed to set max protocol version: {e}"))?;

        if !self.ciphers.is_empty() {
            let cipher_list = self.ciphers.join(":");
            ctx_builder
                .set_cipher_list(&cipher_list)
                .map_err(|e| anyhow!("failed to set cipher list: {e}"))?;
        }

        if let Some(cert_pair) = &self.client_cert_pair {
            cert_pair.add_to_client_ssl_context(&mut ctx_builder)?;
        }

        Ok(ctx_builder)
    }

    fn new_default_builder(&self) -> anyhow::Result<SslConnectorBuilder> {
        let mut ctx_builder = SslConnector::builder(SslMethod::tls_client())
            .map_err(|e| anyhow!("failed to create ssl context builder: {e}"))?;
        ctx_builder.set_verify(SslVerifyMode::PEER);

        if let Some(cert_pair) = &self.client_cert_pair {
            cert_pair.add_to_client_ssl_context(&mut ctx_builder)?;
        }

        Ok(ctx_builder)
    }

    pub fn build_with_alpn_protocols(
        &self,
        alpn_protocols: Option<Vec<AlpnProtocol>>,
    ) -> anyhow::Result<OpensslClientConfig> {
        let mut ctx_builder = match self.protocol {
            Some(OpensslProtocol::Ssl3) => self.new_versioned_builder(SslVersion::SSL3)?,
            Some(OpensslProtocol::Tls1) => self.new_versioned_builder(SslVersion::TLS1)?,
            Some(OpensslProtocol::Tls11) => self.new_versioned_builder(SslVersion::TLS1_1)?,
            Some(OpensslProtocol::Tls12) => self.new_versioned_builder(SslVersion::TLS1_2)?,
            Some(OpensslProtocol::Tls13) => self.new_tls13_builder()?,
            #[cfg(feature = "tongsuo")]
            Some(OpensslProtocol::Tlcp11) => self.new_tlcp_builder()?,
            None => self.new_default_builder()?,
        };

        let mut store_builder = X509StoreBuilder::new()
            .map_err(|e| anyhow!("failed to create ca cert store builder: {e}"))?;
        if !self.no_default_ca_certs {
            store_builder
                .set_default_paths()
                .map_err(|e| anyhow!("failed to load default ca certs: {e}"))?;
        }
        for (i, cert) in self.ca_certs.iter().enumerate() {
            let ca_cert = X509::from_der(cert.as_slice()).unwrap();
            store_builder
                .add_cert(ca_cert)
                .map_err(|e| anyhow!("failed to add ca certificate #{i}: {e}"))?;
        }
        ctx_builder
            .set_verify_cert_store(store_builder.build())
            .map_err(|e| anyhow!("failed to set ca certs: {e}"))?;

        let session_cache = self.session_cache.set_for_client(&mut ctx_builder)?;

        if let Some(protocols) = alpn_protocols {
            let mut len: usize = 0;
            protocols
                .iter()
                .for_each(|p| len += p.wired_identification_sequence().len());
            let mut buf = Vec::with_capacity(len);
            protocols
                .iter()
                .for_each(|p| buf.extend_from_slice(p.wired_identification_sequence()));
            ctx_builder
                .set_alpn_protos(buf.as_slice())
                .map_err(|e| anyhow!("failed to set alpn protocols: {e}"))?;
        }

        Ok(OpensslClientConfig {
            disable_sni: self.disable_sni,
            ssl_context: ctx_builder.build().into_context(),
            handshake_timeout: self.handshake_timeout,
            session_cache,
        })
    }

    pub fn build(&self) -> anyhow::Result<OpensslClientConfig> {
        self.build_with_alpn_protocols(None)
    }
}
