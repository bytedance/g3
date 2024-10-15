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
use openssl::ssl::{Ssl, SslConnector, SslContext, SslContextBuilder, SslMethod, SslVerifyMode};
use openssl::x509::store::X509StoreBuilder;
use openssl::x509::X509;

use super::{
    OpensslClientSessionCache, OpensslSessionCacheConfig, DEFAULT_HANDSHAKE_TIMEOUT,
    MINIMAL_HANDSHAKE_TIMEOUT,
};
use crate::net::{TlsAlpn, TlsServerName, TlsVersion, UpstreamAddr};

#[derive(Clone)]
struct ContextPair {
    ssl_context: SslContext,
    session_cache: Option<OpensslClientSessionCache>,
}

impl ContextPair {
    pub fn build_ssl(
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

#[derive(Clone)]
pub struct OpensslInterceptionClientConfig {
    ssl_context_pair: ContextPair,
    #[cfg(feature = "tongsuo")]
    tlcp_context_pair: ContextPair,
    pub handshake_timeout: Duration,
}

impl OpensslInterceptionClientConfig {
    pub fn build_ssl(
        &self,
        server_name: Option<&TlsServerName>,
        upstream: &UpstreamAddr,
        alpn_ext: Option<&TlsAlpn>,
    ) -> anyhow::Result<Ssl> {
        self.ssl_context_pair
            .build_ssl(server_name, upstream, alpn_ext)
    }

    #[cfg(feature = "tongsuo")]
    pub fn build_tlcp(
        &self,
        server_name: Option<&TlsServerName>,
        upstream: &UpstreamAddr,
        alpn_ext: Option<&TlsAlpn>,
    ) -> anyhow::Result<Ssl> {
        self.tlcp_context_pair
            .build_ssl(server_name, upstream, alpn_ext)
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct OpensslInterceptionClientConfigBuilder {
    min_tls_version: Option<TlsVersion>,
    max_tls_version: Option<TlsVersion>,
    ca_certs: Vec<Vec<u8>>,
    no_default_ca_certs: bool,
    handshake_timeout: Duration,
    session_cache: OpensslSessionCacheConfig,
    supported_groups: String,
    use_ocsp_stapling: bool,
    enable_sct: bool,
    #[cfg(any(feature = "aws-lc", feature = "boringssl"))]
    enable_grease: bool,
    #[cfg(any(feature = "aws-lc", feature = "boringssl"))]
    permute_extensions: bool,
}

impl Default for OpensslInterceptionClientConfigBuilder {
    fn default() -> Self {
        OpensslInterceptionClientConfigBuilder {
            min_tls_version: None,
            max_tls_version: None,
            ca_certs: Vec::new(),
            no_default_ca_certs: false,
            handshake_timeout: DEFAULT_HANDSHAKE_TIMEOUT,
            session_cache: OpensslSessionCacheConfig::new_for_many(),
            supported_groups: String::default(),
            use_ocsp_stapling: false,
            enable_sct: false,
            #[cfg(any(feature = "aws-lc", feature = "boringssl"))]
            enable_grease: false,
            #[cfg(any(feature = "aws-lc", feature = "boringssl"))]
            permute_extensions: false,
        }
    }
}

impl OpensslInterceptionClientConfigBuilder {
    pub fn check(&mut self) -> anyhow::Result<()> {
        if self.handshake_timeout < MINIMAL_HANDSHAKE_TIMEOUT {
            self.handshake_timeout = MINIMAL_HANDSHAKE_TIMEOUT;
        }

        Ok(())
    }

    pub fn set_min_tls_version(&mut self, version: TlsVersion) {
        self.min_tls_version = Some(version);
    }

    pub fn set_max_tls_version(&mut self, version: TlsVersion) {
        self.max_tls_version = Some(version);
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

    #[inline]
    pub fn set_no_session_cache(&mut self) {
        self.session_cache.set_no_session_cache();
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
    pub fn set_enable_sct(&mut self, enable: bool) {
        self.enable_sct = enable;
    }

    #[inline]
    #[cfg(any(feature = "aws-lc", feature = "boringssl"))]
    pub fn set_enable_grease(&mut self, enable: bool) {
        self.enable_grease = enable;
    }

    #[cfg(not(any(feature = "aws-lc", feature = "boringssl")))]
    pub fn set_enable_grease(&mut self, _enable: bool) {
        log::warn!("grease can only be set for BoringSSL variants");
    }

    #[cfg(any(feature = "aws-lc", feature = "boringssl"))]
    pub fn set_permute_extensions(&mut self, enable: bool) {
        self.permute_extensions = enable;
    }

    #[cfg(not(any(feature = "aws-lc", feature = "boringssl")))]
    pub fn set_permute_extensions(&mut self, _enable: bool) {
        log::warn!("permute extensions can only be set for BoringSSL variants");
    }

    fn build_set_tls_version(&self, ctx_builder: &mut SslContextBuilder) -> anyhow::Result<()> {
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
        Ok(())
    }

    fn build_set_verify_cert_store(
        &self,
        ctx_builder: &mut SslContextBuilder,
    ) -> anyhow::Result<()> {
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
        #[cfg(not(feature = "boringssl"))]
        ctx_builder
            .set_verify_cert_store(store_builder.build())
            .map_err(|e| anyhow!("failed to set ca certs: {e}"))?;
        #[cfg(feature = "boringssl")]
        ctx_builder.set_cert_store(store_builder.build());
        Ok(())
    }

    #[cfg(any(feature = "aws-lc", feature = "boringssl", feature = "tongsuo"))]
    fn build_set_cert_compression(
        &self,
        ctx_builder: &mut SslContextBuilder,
    ) -> anyhow::Result<()> {
        use openssl::ssl::CertCompressionAlgorithm;

        ctx_builder
            .add_cert_decompression_alg(CertCompressionAlgorithm::BROTLI, |in_buf, out_buf| {
                use std::io::Read;

                brotli::Decompressor::new(in_buf, 4096)
                    .read(out_buf)
                    .unwrap_or(0)
            })
            .map_err(|e| anyhow!("failed to set brotli cert decompression algorithm: {e}"))?;

        Ok(())
    }

    #[cfg(any(feature = "aws-lc", feature = "boringssl"))]
    fn build_ssl_context(&self) -> anyhow::Result<ContextPair> {
        let mut ctx_builder = SslConnector::builder(SslMethod::tls_client())
            .map_err(|e| anyhow!("failed to create ssl context builder: {e}"))?;
        ctx_builder.set_verify(SslVerifyMode::PEER);

        self.build_set_tls_version(&mut ctx_builder)?;

        if !self.supported_groups.is_empty() {
            ctx_builder
                .set_groups_list(&self.supported_groups)
                .map_err(|e| anyhow!("failed to set supported elliptic curve groups: {e}"))?;
        }

        if self.use_ocsp_stapling {
            ctx_builder.enable_ocsp_stapling();
            // TODO check OCSP response
        }

        if self.enable_sct {
            ctx_builder.enable_signed_cert_timestamps();
            // TODO check SCT list for AWS-LC or BoringSSL
        }

        if self.enable_grease {
            ctx_builder.set_grease_enabled(true);
        }
        if self.permute_extensions {
            ctx_builder.set_permute_extensions(true);
        }

        self.build_set_cert_compression(&mut ctx_builder)?;

        self.build_set_verify_cert_store(&mut ctx_builder)?;

        let session_cache = self.session_cache.set_for_client(&mut ctx_builder)?;

        Ok(ContextPair {
            ssl_context: ctx_builder.build().into_context(),
            session_cache,
        })
    }

    #[cfg(not(any(feature = "aws-lc", feature = "boringssl")))]
    fn build_ssl_context(&self) -> anyhow::Result<ContextPair> {
        use openssl::ssl::{SslCtValidationMode, StatusType};

        let mut ctx_builder = SslConnector::builder(SslMethod::tls_client())
            .map_err(|e| anyhow!("failed to create ssl context builder: {e}"))?;
        ctx_builder.set_verify(SslVerifyMode::PEER);

        self.build_set_tls_version(&mut ctx_builder)?;

        if !self.supported_groups.is_empty() {
            ctx_builder
                .set_groups_list(&self.supported_groups)
                .map_err(|e| anyhow!("failed to set supported elliptic curve groups: {e}"))?;
        }

        if self.use_ocsp_stapling {
            ctx_builder
                .set_status_type(StatusType::OCSP)
                .map_err(|e| anyhow!("failed to enable OCSP status request: {e}"))?;
            // TODO check OCSP response
        }

        if self.enable_sct {
            ctx_builder
                .enable_ct(SslCtValidationMode::PERMISSIVE)
                .map_err(|e| anyhow!("failed to enable SCT: {e}"))?;
        }

        #[cfg(feature = "tongsuo")]
        self.build_set_cert_compression(&mut ctx_builder)?;

        self.build_set_verify_cert_store(&mut ctx_builder)?;

        let session_cache = self.session_cache.set_for_client(&mut ctx_builder)?;

        Ok(ContextPair {
            ssl_context: ctx_builder.build().into_context(),
            session_cache,
        })
    }

    #[cfg(feature = "tongsuo")]
    fn build_tlcp_context(&self) -> anyhow::Result<ContextPair> {
        use openssl::ssl::{SslCtValidationMode, StatusType};

        let mut ctx_builder = SslConnector::builder(SslMethod::ntls_client())
            .map_err(|e| anyhow!("failed to create tlcp context builder: {e}"))?;
        ctx_builder.set_verify(SslVerifyMode::PEER);

        if !self.supported_groups.is_empty() {
            ctx_builder
                .set_groups_list(&self.supported_groups)
                .map_err(|e| anyhow!("failed to set supported elliptic curve groups: {e}"))?;
        }

        if self.use_ocsp_stapling {
            ctx_builder
                .set_status_type(StatusType::OCSP)
                .map_err(|e| anyhow!("failed to enable OCSP status request: {e}"))?;
            // TODO check OCSP response
        }

        if self.enable_sct {
            ctx_builder
                .enable_ct(SslCtValidationMode::PERMISSIVE)
                .map_err(|e| anyhow!("failed to enable SCT: {e}"))?;
        }

        self.build_set_cert_compression(&mut ctx_builder)?;

        self.build_set_verify_cert_store(&mut ctx_builder)?;

        let session_cache = self.session_cache.set_for_client(&mut ctx_builder)?;

        Ok(ContextPair {
            ssl_context: ctx_builder.build().into_context(),
            session_cache,
        })
    }

    pub fn build(&self) -> anyhow::Result<OpensslInterceptionClientConfig> {
        Ok(OpensslInterceptionClientConfig {
            ssl_context_pair: self.build_ssl_context()?,
            #[cfg(feature = "tongsuo")]
            tlcp_context_pair: self.build_tlcp_context()?,
            handshake_timeout: self.handshake_timeout,
        })
    }
}
