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
use openssl::ssl::{Ssl, SslConnector, SslContext, SslMethod, SslVerifyMode};
use openssl::x509::store::X509StoreBuilder;
use openssl::x509::X509;

use super::{
    OpensslClientSessionCache, OpensslSessionCacheConfig, DEFAULT_HANDSHAKE_TIMEOUT,
    MINIMAL_HANDSHAKE_TIMEOUT,
};
use crate::net::UpstreamAddr;

#[derive(Clone)]
pub struct OpensslInterceptionClientConfig {
    ssl_context: SslContext,
    pub handshake_timeout: Duration,
    session_cache: Option<OpensslClientSessionCache>,
}

impl OpensslInterceptionClientConfig {
    pub fn build_ssl<'a>(
        &'a self,
        sni_hostname: Option<&str>,
        upstream: &UpstreamAddr,
        alpn_protocols: Option<impl Iterator<Item = &'a [u8]>>,
    ) -> anyhow::Result<Ssl> {
        let mut ssl =
            Ssl::new(&self.ssl_context).map_err(|e| anyhow!("failed to get new Ssl state: {e}"))?;
        if let Some(domain) = sni_hostname {
            let verify_param = ssl.param_mut();
            verify_param
                .set_host(domain)
                .map_err(|e| anyhow!("failed to set cert verify domain: {e}"))?;
            ssl.set_hostname(domain)
                .map_err(|e| anyhow!("failed to set sni hostname: {e}"))?;
        }
        if let Some(cache) = &self.session_cache {
            cache.find_and_set_cache(&mut ssl, upstream.host(), upstream.port())?;
        }
        if let Some(protocols) = alpn_protocols {
            let mut buf = Vec::with_capacity(32);
            protocols.for_each(|p| {
                if let Ok(len) = u8::try_from(p.len()) {
                    buf.push(len);
                    buf.extend_from_slice(p);
                }
            });
            ssl.set_alpn_protos(buf.as_slice())
                .map_err(|e| anyhow!("failed to set alpn protocols: {e}"))?;
        }
        Ok(ssl)
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct OpensslInterceptionClientConfigBuilder {
    ca_certs: Vec<Vec<u8>>,
    no_default_ca_certs: bool,
    handshake_timeout: Duration,
    session_cache: OpensslSessionCacheConfig,
}

impl Default for OpensslInterceptionClientConfigBuilder {
    fn default() -> Self {
        OpensslInterceptionClientConfigBuilder {
            ca_certs: Vec::new(),
            no_default_ca_certs: false,
            handshake_timeout: DEFAULT_HANDSHAKE_TIMEOUT,
            session_cache: OpensslSessionCacheConfig::default(),
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

    pub fn build(&self) -> anyhow::Result<OpensslInterceptionClientConfig> {
        let mut ctx_builder = SslConnector::builder(SslMethod::tls_client())
            .map_err(|e| anyhow!("failed to create ssl context builder: {e}"))?;
        ctx_builder.set_verify(SslVerifyMode::PEER);

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

        Ok(OpensslInterceptionClientConfig {
            ssl_context: ctx_builder.build().into_context(),
            handshake_timeout: self.handshake_timeout,
            session_cache,
        })
    }
}
