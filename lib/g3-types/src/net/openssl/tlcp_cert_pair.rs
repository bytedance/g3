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

use anyhow::anyhow;
use openssl::pkey::{PKey, Private};
use openssl::ssl::SslContextBuilder;
use openssl::x509::X509;

use super::OpensslSessionIdContext;

#[derive(Default, Clone, Debug, Eq, PartialEq)]
pub struct OpensslTlcpCertificatePair {
    enc_leaf_cert: Vec<u8>,
    sign_leaf_cert: Vec<u8>,
    chain_certs: Vec<Vec<u8>>,
    enc_key: Vec<u8>,
    sign_key: Vec<u8>,
}

impl OpensslTlcpCertificatePair {
    pub fn check(&self) -> anyhow::Result<()> {
        if self.sign_leaf_cert.is_empty() {
            return Err(anyhow!("no sign certificate set"));
        }
        if self.enc_leaf_cert.is_empty() {
            return Err(anyhow!("no enc certificate set"));
        }
        if self.enc_key.is_empty() {
            return Err(anyhow!("no enc private key set"));
        }
        if self.sign_key.is_empty() {
            return Err(anyhow!("no sign private key set"));
        }
        Ok(())
    }

    pub fn set_sign_certificates(&mut self, certs: Vec<X509>) -> anyhow::Result<()> {
        let mut certs_iter = certs.into_iter();
        let leaf_cert = certs_iter
            .next()
            .ok_or_else(|| anyhow!("no sign certificate found"))?;
        let leaf_cert_der = leaf_cert
            .to_der()
            .map_err(|e| anyhow!("failed to encode sign certificate: {e}"))?;
        self.sign_leaf_cert = leaf_cert_der;

        for (i, cert) in certs_iter.enumerate() {
            let bytes = cert
                .to_der()
                .map_err(|e| anyhow!("failed to encode chain certificate #{i}: {e}"))?;
            self.chain_certs.push(bytes);
        }

        Ok(())
    }

    pub fn set_enc_certificates(&mut self, certs: Vec<X509>) -> anyhow::Result<()> {
        let mut certs_iter = certs.into_iter();
        let leaf_cert = certs_iter
            .next()
            .ok_or_else(|| anyhow!("no enc certificate found"))?;
        let leaf_cert_der = leaf_cert
            .to_der()
            .map_err(|e| anyhow!("failed to encode enc certificate: {e}"))?;
        self.enc_leaf_cert = leaf_cert_der;

        for (i, cert) in certs_iter.enumerate() {
            let bytes = cert
                .to_der()
                .map_err(|e| anyhow!("failed to encode chain certificate #{i}: {e}"))?;
            self.chain_certs.push(bytes);
        }

        Ok(())
    }

    pub fn set_sign_private_key(&mut self, key: PKey<Private>) -> anyhow::Result<()> {
        let key_der = key
            .private_key_to_der()
            .map_err(|e| anyhow!("failed to encode private key: {e}"))?;
        self.sign_key = key_der;
        Ok(())
    }

    pub fn set_enc_private_key(&mut self, key: PKey<Private>) -> anyhow::Result<()> {
        let key_der = key
            .private_key_to_der()
            .map_err(|e| anyhow!("failed to encode private key: {e}"))?;
        self.enc_key = key_der;
        Ok(())
    }

    pub fn add_to_client_ssl_context(
        &self,
        ssl_builder: &mut SslContextBuilder,
    ) -> anyhow::Result<()> {
        let leaf_cert = X509::from_der(self.sign_leaf_cert.as_slice()).unwrap();
        ssl_builder
            .set_sign_certificate(&leaf_cert)
            .map_err(|e| anyhow!("failed to set sign certificate: {e}"))?;

        let leaf_cert = X509::from_der(self.enc_leaf_cert.as_slice()).unwrap();
        ssl_builder
            .set_enc_certificate(&leaf_cert)
            .map_err(|e| anyhow!("failed to set sign certificate: {e}"))?;

        self.add_to_ssl_context(ssl_builder)
    }

    pub fn add_to_server_ssl_context(
        &self,
        ssl_builder: &mut SslContextBuilder,
        id_ctx: &mut OpensslSessionIdContext,
    ) -> anyhow::Result<()> {
        let leaf_cert = X509::from_der(self.sign_leaf_cert.as_slice()).unwrap();
        ssl_builder
            .set_sign_certificate(&leaf_cert)
            .map_err(|e| anyhow!("failed to set sign certificate: {e}"))?;
        id_ctx
            .add_cert(&leaf_cert)
            .map_err(|e| anyhow!("failed to add sign cert to session id context: {e}"))?;

        let leaf_cert = X509::from_der(self.enc_leaf_cert.as_slice()).unwrap();
        ssl_builder
            .set_enc_certificate(&leaf_cert)
            .map_err(|e| anyhow!("failed to set sign certificate: {e}"))?;
        id_ctx
            .add_cert(&leaf_cert)
            .map_err(|e| anyhow!("failed to add enc cert to session id context: {e}"))?;

        self.add_to_ssl_context(ssl_builder)
    }

    fn add_to_ssl_context(&self, ssl_builder: &mut SslContextBuilder) -> anyhow::Result<()> {
        for (i, cert) in self.chain_certs.iter().enumerate() {
            let chain_cert = X509::from_der(cert.as_slice()).unwrap();
            ssl_builder
                .add_extra_chain_cert(chain_cert)
                .map_err(|e| anyhow!("failed to add chain certificate #{i}: {e}"))?;
        }

        let key = PKey::private_key_from_der(self.sign_key.as_slice()).unwrap();
        ssl_builder
            .set_sign_private_key(&key)
            .map_err(|e| anyhow!("failed to set sign private key: {e}"))?;
        let key = PKey::private_key_from_der(self.enc_key.as_slice()).unwrap();
        ssl_builder
            .set_enc_private_key(&key)
            .map_err(|e| anyhow!("failed to set private key: {e}"))?;
        Ok(())
    }
}
