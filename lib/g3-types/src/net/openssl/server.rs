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

use anyhow::{anyhow, Context};
use bytes::BufMut;
use openssl::ssl::{
    SslAcceptor, SslAcceptorBuilder, SslContext, SslMethod, SslSessionCacheMode, SslVerifyMode,
};
use openssl::stack::Stack;
use openssl::x509::store::X509StoreBuilder;
use openssl::x509::X509;

use super::OpensslCertificatePair;
#[cfg(feature = "vendored-tongsuo")]
use super::OpensslTlcpCertificatePair;
use crate::net::AlpnProtocol;

#[cfg(feature = "vendored-tongsuo")]
const TLS_DEFAULT_CIPHER_SUITES: &str =
    "TLS_AES_128_GCM_SHA256:TLS_AES_256_GCM_SHA384:TLS_CHACHA20_POLY1305_SHA256:TLS_SM4_GCM_SM3";
#[cfg(feature = "vendored-tongsuo")]
const TLS_DEFAULT_CIPHER_LIST: &str =
    "ECDHE-ECDSA-AES128-GCM-SHA256:ECDHE-RSA-AES128-GCM-SHA256:ECDHE-ECDSA-AES256-GCM-SHA384:\
     ECDHE-RSA-AES256-GCM-SHA384:ECDHE-ECDSA-CHACHA20-POLY1305:ECDHE-RSA-CHACHA20-POLY1305:\
     DHE-RSA-AES128-GCM-SHA256:DHE-RSA-AES256-GCM-SHA384";
#[cfg(feature = "vendored-tongsuo")]
const TLCP_DEFAULT_CIPHER_LIST: &str = "ECDHE-SM2-WITH-SM4-SM3:ECC-SM2-WITH-SM4-SM3:\
     ECDHE-SM2-SM4-CBC-SM3:ECDHE-SM2-SM4-GCM-SM3:ECC-SM2-SM4-CBC-SM3:ECC-SM2-SM4-GCM-SM3:\
     RSA-SM4-CBC-SM3:RSA-SM4-GCM-SM3:RSA-SM4-CBC-SHA256:RSA-SM4-GCM-SHA256";

#[derive(Clone)]
pub struct OpensslServerConfig {
    pub ssl_context: SslContext,
    pub accept_timeout: Duration,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct OpensslServerConfigBuilder {
    cert_pairs: Vec<OpensslCertificatePair>,
    #[cfg(feature = "vendored-tongsuo")]
    tlcp_cert_pairs: Vec<OpensslTlcpCertificatePair>,
    client_auth: bool,
    client_auth_certs: Vec<Vec<u8>>,
    accept_timeout: Duration,
}

impl OpensslServerConfigBuilder {
    pub fn empty() -> Self {
        OpensslServerConfigBuilder {
            cert_pairs: Vec::with_capacity(1),
            #[cfg(feature = "vendored-tongsuo")]
            tlcp_cert_pairs: Vec::with_capacity(1),
            client_auth: false,
            client_auth_certs: Vec::new(),
            accept_timeout: Duration::from_secs(10),
        }
    }

    #[cfg(not(feature = "vendored-tongsuo"))]
    pub fn check(&self) -> anyhow::Result<()> {
        if self.cert_pairs.is_empty() {
            return Err(anyhow!("no cert pair is set"));
        }

        Ok(())
    }

    #[cfg(feature = "vendored-tongsuo")]
    pub fn check(&self) -> anyhow::Result<()> {
        if self.cert_pairs.is_empty() && self.tlcp_cert_pairs.is_empty() {
            return Err(anyhow!("no cert pair is set"));
        }

        Ok(())
    }

    pub fn enable_client_auth(&mut self) {
        self.client_auth = true;
    }

    pub fn set_client_auth_certificates(&mut self, certs: Vec<X509>) -> anyhow::Result<()> {
        for (i, cert) in certs.into_iter().enumerate() {
            let bytes = cert
                .to_der()
                .map_err(|e| anyhow!("failed to encode client chain certificate #{i}: {e}"))?;
            self.client_auth_certs.push(bytes);
        }
        Ok(())
    }

    pub fn push_cert_pair(&mut self, cert_pair: OpensslCertificatePair) -> anyhow::Result<()> {
        cert_pair.check()?;
        self.cert_pairs.push(cert_pair);
        Ok(())
    }

    #[cfg(feature = "vendored-tongsuo")]
    pub fn push_tlcp_cert_pair(
        &mut self,
        cert_pair: OpensslTlcpCertificatePair,
    ) -> anyhow::Result<()> {
        cert_pair.check()?;
        self.tlcp_cert_pairs.push(cert_pair);
        Ok(())
    }

    pub fn set_accept_timeout(&mut self, timeout: Duration) {
        self.accept_timeout = timeout;
    }

    fn build_tls_acceptor(&self) -> anyhow::Result<SslAcceptorBuilder> {
        let mut ssl_builder = SslAcceptor::mozilla_intermediate_v5(SslMethod::tls_server())
            .map_err(|e| anyhow!("failed to build ssl context: {e}"))?;

        #[cfg(feature = "vendored-tongsuo")]
        ssl_builder
            .set_ciphersuites(TLS_DEFAULT_CIPHER_SUITES)
            .map_err(|e| anyhow!("failed to set tls1.3 cipher suites: {e}"))?;

        for (i, pair) in self.cert_pairs.iter().enumerate() {
            pair.add_to_ssl_context(&mut ssl_builder)
                .context(format!("failed to add cert pair #{i} to ssl context"))?;
        }

        Ok(ssl_builder)
    }

    #[cfg(feature = "vendored-tongsuo")]
    fn build_tlcp_acceptor(&self) -> anyhow::Result<SslAcceptorBuilder> {
        let mut ssl_builder = SslAcceptor::mozilla_intermediate_v5(SslMethod::ntls_server())
            .map_err(|e| anyhow!("failed to build ssl context: {e}"))?;
        ssl_builder.enable_force_ntls();

        ssl_builder
            .set_cipher_list(TLCP_DEFAULT_CIPHER_LIST)
            .map_err(|e| anyhow!("failed to set tlcp cipher list: {e}"))?;

        for (i, pair) in self.tlcp_cert_pairs.iter().enumerate() {
            pair.add_to_ssl_context(&mut ssl_builder)
                .context(format!("failed to add cert pair #{i} to ssl context"))?;
        }

        Ok(ssl_builder)
    }

    #[cfg(feature = "vendored-tongsuo")]
    fn build_acceptor(&self) -> anyhow::Result<SslAcceptorBuilder> {
        if self.tlcp_cert_pairs.is_empty() {
            return self.build_tls_acceptor();
        }

        if self.cert_pairs.is_empty() {
            return self.build_tlcp_acceptor();
        }

        let mut ssl_builder = SslAcceptor::mozilla_intermediate_v5(SslMethod::ntls_server())
            .map_err(|e| anyhow!("failed to build ssl context: {e}"))?;
        ssl_builder.enable_ntls();

        ssl_builder
            .set_cipher_list(&format!(
                "{TLS_DEFAULT_CIPHER_LIST}:{TLCP_DEFAULT_CIPHER_LIST}"
            ))
            .map_err(|e| anyhow!("failed to set tls1.2 / tlcp cipher list: {e}"))?;
        ssl_builder
            .set_ciphersuites(TLS_DEFAULT_CIPHER_SUITES)
            .map_err(|e| anyhow!("failed to set tls1.3 cipher suites: {e}"))?;

        for (i, pair) in self.tlcp_cert_pairs.iter().enumerate() {
            pair.add_to_ssl_context(&mut ssl_builder)
                .context(format!("failed to add cert pair #{i} to ssl context"))?;
        }
        for (i, pair) in self.cert_pairs.iter().enumerate() {
            pair.add_to_ssl_context(&mut ssl_builder)
                .context(format!("failed to add cert pair #{i} to ssl context"))?;
        }

        Ok(ssl_builder)
    }

    #[cfg(not(feature = "vendored-tongsuo"))]
    fn build_acceptor(&self) -> anyhow::Result<SslAcceptorBuilder> {
        self.build_tls_acceptor()
    }

    pub fn build_with_alpn_protocols(
        &self,
        alpn_protocols: Option<Vec<AlpnProtocol>>,
    ) -> anyhow::Result<OpensslServerConfig> {
        let mut ssl_builder = self.build_acceptor()?;

        ssl_builder.set_session_cache_mode(SslSessionCacheMode::SERVER);

        if self.client_auth {
            ssl_builder.set_verify(SslVerifyMode::PEER | SslVerifyMode::FAIL_IF_NO_PEER_CERT);

            let mut store_builder = X509StoreBuilder::new()
                .map_err(|e| anyhow!("failed to create ca cert store builder: {e}"))?;
            if self.client_auth_certs.is_empty() {
                store_builder
                    .set_default_paths()
                    .map_err(|e| anyhow!("failed to load default ca certs: {e}"))?;
            } else {
                for (i, cert) in self.client_auth_certs.iter().enumerate() {
                    let ca_cert = X509::from_der(cert.as_slice()).unwrap();
                    store_builder
                        .add_cert(ca_cert)
                        .map_err(|e| anyhow!("[#{i}] failed to add ca certificate: {e}"))?;
                }
            }
            let store = store_builder.build();

            let mut ca_stack =
                Stack::new().map_err(|e| anyhow!("failed to get new ca name stack: {e}"))?;
            for (i, cert) in store.all_certificates().iter().enumerate() {
                let name = cert
                    .subject_name()
                    .to_owned()
                    .map_err(|e| anyhow!("[#{i}] failed to get subject name: {e}"))?;
                ca_stack
                    .push(name)
                    .map_err(|e| anyhow!("[#{i}] failed to push to ca name stack: {e}"))?;
            }

            ssl_builder.set_client_ca_list(ca_stack);
            ssl_builder
                .set_verify_cert_store(store)
                .map_err(|e| anyhow!("failed to set ca certs: {e}"))?;
        } else {
            ssl_builder.set_verify(SslVerifyMode::NONE);
        }

        // ssl_builder.set_mode() // TODO do we need it?
        // ssl_builder.set_options() // TODO do we need it?

        if let Some(protocols) = alpn_protocols {
            let mut buf = Vec::with_capacity(32);
            protocols.iter().for_each(|p| {
                buf.put_slice(p.wired_identification_sequence());
            });
            if !buf.is_empty() {
                ssl_builder
                    .set_alpn_protos(buf.as_slice())
                    .map_err(|e| anyhow!("failed to set alpn protocols: {e}"))?;
            }
        }

        let ssl_acceptor = ssl_builder.build();

        Ok(OpensslServerConfig {
            ssl_context: ssl_acceptor.into_context(),
            accept_timeout: self.accept_timeout,
        })
    }

    #[inline]
    pub fn build(&self) -> anyhow::Result<OpensslServerConfig> {
        self.build_with_alpn_protocols(None)
    }
}
