/*
 * SPDX-License-Identifier: Apache-2.0
 * Copyright 2023-2025 ByteDance and/or its affiliates.
 */

use std::time::Duration;

use anyhow::anyhow;
use log::warn;
#[cfg(any(boringssl, tongsuo))]
use openssl::ssl::CertCompressionAlgorithm;
#[cfg(not(any(boringssl, libressl)))]
use openssl::ssl::SslCtValidationMode;
#[cfg(not(boringssl))]
use openssl::ssl::StatusType;
use openssl::ssl::{
    Ssl, SslConnector, SslConnectorBuilder, SslContext, SslMethod, SslVerifyMode, SslVersion,
};
use openssl::x509::X509;
use openssl::x509::store::X509StoreBuilder;

use super::{OpensslCertificatePair, OpensslProtocol, OpensslTlcpCertificatePair};
use crate::net::tls::AlpnProtocol;
use crate::net::{Host, TlsAlpn, TlsServerName, TlsVersion, UpstreamAddr};

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

    pub fn build_mimic_ssl(
        &self,
        server_name: Option<&TlsServerName>,
        upstream: &UpstreamAddr,
        alpn_ext: Option<&TlsAlpn>,
    ) -> anyhow::Result<Ssl> {
        let mut ssl =
            Ssl::new(&self.ssl_context).map_err(|e| anyhow!("failed to get new Ssl state: {e}"))?;
        if let Some(name) = server_name {
            let verify_param = ssl.param_mut();
            verify_param
                .set_host(name.as_ref())
                .map_err(|e| anyhow!("failed to set cert verify domain: {e}"))?;
            ssl.set_hostname(name.as_ref())
                .map_err(|e| anyhow!("failed to set sni hostname: {e}"))?;
        }
        if let Some(cache) = &self.session_cache {
            cache.find_and_set_cache(&mut ssl, upstream.host(), upstream.port())?;
        }
        if let Some(v) = alpn_ext {
            ssl.set_alpn_protos(v.wired_list_sequence())
                .map_err(|e| anyhow!("failed to set alpn protocols: {e}"))?;
        }
        Ok(ssl)
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct OpensslClientConfigBuilder {
    protocol: Option<OpensslProtocol>,
    min_tls_version: Option<TlsVersion>,
    max_tls_version: Option<TlsVersion>,
    ciphers: Vec<String>,
    disable_sni: bool,
    ca_certs: Vec<Vec<u8>>,
    no_default_ca_certs: bool,
    client_cert_pair: Option<OpensslCertificatePair>,
    client_tlcp_cert_pair: Option<OpensslTlcpCertificatePair>,
    handshake_timeout: Duration,
    session_cache: OpensslSessionCacheConfig,
    supported_groups: String,
    use_ocsp_stapling: bool,
    #[cfg(not(libressl))]
    enable_sct: bool,
    #[cfg(boringssl)]
    enable_grease: bool,
    #[cfg(boringssl)]
    permute_extensions: bool,
    insecure: bool,
}

impl Default for OpensslClientConfigBuilder {
    fn default() -> Self {
        OpensslClientConfigBuilder {
            protocol: None,
            min_tls_version: None,
            max_tls_version: None,
            ciphers: Vec::new(),
            disable_sni: false,
            ca_certs: Vec::new(),
            no_default_ca_certs: false,
            client_cert_pair: None,
            client_tlcp_cert_pair: None,
            handshake_timeout: DEFAULT_HANDSHAKE_TIMEOUT,
            session_cache: OpensslSessionCacheConfig::default(),
            supported_groups: String::default(),
            use_ocsp_stapling: false,
            #[cfg(not(libressl))]
            enable_sct: false,
            #[cfg(boringssl)]
            enable_grease: false,
            #[cfg(boringssl)]
            permute_extensions: false,
            insecure: false,
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

    pub fn set_min_tls_version(&mut self, version: TlsVersion) {
        self.min_tls_version = Some(version);
    }

    pub fn set_max_tls_version(&mut self, version: TlsVersion) {
        self.max_tls_version = Some(version);
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

    #[inline]
    pub fn set_supported_groups(&mut self, groups: String) {
        self.supported_groups = groups;
    }

    #[inline]
    pub fn set_use_ocsp_stapling(&mut self, enable: bool) {
        self.use_ocsp_stapling = enable;
    }

    #[inline]
    #[cfg(not(libressl))]
    pub fn set_enable_sct(&mut self, enable: bool) {
        self.enable_sct = enable;
    }

    #[inline]
    #[cfg(libressl)]
    pub fn set_enable_sct(&mut self, _enable: bool) {
        warn!("SCT can not be enabled for LibreSSL");
    }

    #[inline]
    #[cfg(boringssl)]
    pub fn set_enable_grease(&mut self, enable: bool) {
        self.enable_grease = enable;
    }

    #[cfg(not(boringssl))]
    pub fn set_enable_grease(&mut self, _enable: bool) {
        warn!("grease can only be set for BoringSSL variants");
    }

    #[cfg(boringssl)]
    pub fn set_permute_extensions(&mut self, enable: bool) {
        self.permute_extensions = enable;
    }

    #[cfg(not(boringssl))]
    pub fn set_permute_extensions(&mut self, _enable: bool) {
        warn!("permute extensions can only be set for BoringSSL variants");
    }

    pub fn set_insecure(&mut self, enable: bool) {
        self.insecure = enable;
    }

    fn set_verify(&self, builder: &mut SslConnectorBuilder) {
        if self.insecure {
            warn!(
                "Tls Insecure Mode: Tls Peer (server) cert vertification is no longer enforced for this Context!"
            );
            builder.set_verify(SslVerifyMode::NONE);
        } else {
            builder.set_verify(SslVerifyMode::PEER);
        }
    }

    #[cfg(tongsuo)]
    fn new_tlcp_builder(&self) -> anyhow::Result<SslConnectorBuilder> {
        let mut ctx_builder = SslConnector::builder(SslMethod::ntls_client())
            .map_err(|e| anyhow!("failed to create ssl context builder: {e}"))?;

        self.set_verify(&mut ctx_builder);

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

    #[cfg(not(boringssl))]
    fn new_tls13_builder(&self) -> anyhow::Result<SslConnectorBuilder> {
        let mut ctx_builder = SslConnector::builder(SslMethod::tls_client())
            .map_err(|e| anyhow!("failed to create ssl context builder: {e}"))?;

        self.set_verify(&mut ctx_builder);

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

    #[cfg(boringssl)]
    fn new_tls13_builder(&self) -> anyhow::Result<SslConnectorBuilder> {
        let mut ctx_builder = SslConnector::builder(SslMethod::tls_client())
            .map_err(|e| anyhow!("failed to create ssl context builder: {e}"))?;

        self.set_verify(&mut ctx_builder);

        ctx_builder
            .set_min_proto_version(Some(SslVersion::TLS1_3))
            .map_err(|e| anyhow!("failed to set min protocol version: {e}"))?;
        ctx_builder
            .set_max_proto_version(Some(SslVersion::TLS1_3))
            .map_err(|e| anyhow!("failed to set max protocol version: {e}"))?;

        if !self.ciphers.is_empty() {
            return Err(anyhow!(
                "boringssl has no support for setting TLS ciphersuites"
            ));
        }

        if let Some(cert_pair) = &self.client_cert_pair {
            cert_pair.add_to_client_ssl_context(&mut ctx_builder)?;
        }

        Ok(ctx_builder)
    }

    fn new_versioned_builder(&self, version: SslVersion) -> anyhow::Result<SslConnectorBuilder> {
        let mut ctx_builder = SslConnector::builder(SslMethod::tls_client())
            .map_err(|e| anyhow!("failed to create ssl context builder: {e}"))?;

        self.set_verify(&mut ctx_builder);

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

        self.set_verify(&mut ctx_builder);

        if let Some(version) = self.min_tls_version {
            ctx_builder
                .set_min_proto_version(Some(version.into()))
                .map_err(|e| anyhow!("failed to set min ssl version to {version}: {e}"))?;
        }
        if let Some(version) = self.max_tls_version {
            ctx_builder
                .set_max_proto_version(Some(version.into()))
                .map_err(|e| anyhow!("failed to set max ssl version to {version}: {e}"))?;
        }

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
            #[cfg(tongsuo)]
            Some(OpensslProtocol::Tlcp11) => self.new_tlcp_builder()?,
            None => self.new_default_builder()?,
        };

        if !self.supported_groups.is_empty() {
            ctx_builder
                .set_groups_list(&self.supported_groups)
                .map_err(|e| anyhow!("failed to set supported elliptic curve groups: {e}"))?;
        }

        if self.use_ocsp_stapling {
            #[cfg(not(boringssl))]
            ctx_builder
                .set_status_type(StatusType::OCSP)
                .map_err(|e| anyhow!("failed to enable OCSP status request: {e}"))?;
            #[cfg(boringssl)]
            ctx_builder.enable_ocsp_stapling();
            // TODO check OCSP response
        }

        #[cfg(not(libressl))]
        if self.enable_sct {
            #[cfg(not(boringssl))]
            ctx_builder
                .enable_ct(SslCtValidationMode::PERMISSIVE)
                .map_err(|e| anyhow!("failed to enable SCT: {e}"))?;
            #[cfg(boringssl)]
            ctx_builder.enable_signed_cert_timestamps();
            // TODO check SCT list for AWS-LC or BoringSSL
        }

        #[cfg(boringssl)]
        if self.enable_grease {
            ctx_builder.set_grease_enabled(true);
        }
        #[cfg(boringssl)]
        if self.permute_extensions {
            ctx_builder.set_permute_extensions(true);
        }

        #[cfg(any(boringssl, tongsuo))]
        ctx_builder
            .add_cert_decompression_alg(CertCompressionAlgorithm::BROTLI, |in_buf, out_buf| {
                use std::io::Read;

                brotli::Decompressor::new(in_buf, 4096)
                    .read(out_buf)
                    .unwrap_or(0)
            })
            .map_err(|e| anyhow!("failed to set cert decompression algorithm: {e}"))?;

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
        #[cfg(not(libressl))]
        ctx_builder
            .set_verify_cert_store(store_builder.build())
            .map_err(|e| anyhow!("failed to set verify ca certs: {e}"))?;
        #[cfg(libressl)]
        ctx_builder.set_cert_store(store_builder.build());

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
