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

#[derive(Default, Clone, Debug, Eq, PartialEq)]
pub struct OpensslCertificatePair {
    leaf_cert: Vec<u8>,
    chain_certs: Vec<Vec<u8>>,
    key: Vec<u8>,
}

impl OpensslCertificatePair {
    pub fn check(&self) -> anyhow::Result<()> {
        if self.leaf_cert.is_empty() {
            return Err(anyhow!("no certificate set"));
        }
        if self.key.is_empty() {
            return Err(anyhow!("no private key set"));
        }
        Ok(())
    }

    pub fn is_set(&self) -> bool {
        !self.leaf_cert.is_empty()
    }

    pub fn set_certificates(&mut self, certs: Vec<X509>) -> anyhow::Result<()> {
        let certs_len = certs.len();

        let mut certs_iter = certs.into_iter();
        let leaf_cert = certs_iter
            .next()
            .ok_or_else(|| anyhow!("no certificate found"))?;
        let leaf_cert_der = leaf_cert
            .to_der()
            .map_err(|e| anyhow!("failed to encode client certificate: {e}"))?;
        self.leaf_cert = leaf_cert_der;

        let mut chain_certs = Vec::with_capacity(certs_len);
        for (i, cert) in certs_iter.enumerate() {
            let bytes = cert
                .to_der()
                .map_err(|e| anyhow!("failed to encode client chain certificate #{i}: {e}"))?;
            chain_certs.push(bytes);
        }
        self.chain_certs = chain_certs;

        Ok(())
    }

    pub fn set_private_key(&mut self, key: PKey<Private>) -> anyhow::Result<()> {
        let key_der = key
            .private_key_to_der()
            .map_err(|e| anyhow!("failed to encode private key: {e}"))?;
        self.key = key_der;
        Ok(())
    }

    pub fn add_to_ssl_context(&self, ssl_builder: &mut SslContextBuilder) -> anyhow::Result<()> {
        let leaf_cert = X509::from_der(self.leaf_cert.as_slice()).unwrap();
        ssl_builder
            .set_certificate(&leaf_cert)
            .map_err(|e| anyhow!("failed to set certificate: {e}"))?;

        for (i, cert) in self.chain_certs.iter().enumerate() {
            let chain_cert = X509::from_der(cert.as_slice()).unwrap();
            ssl_builder
                .add_extra_chain_cert(chain_cert)
                .map_err(|e| anyhow!("failed to add chain certificate #{i}: {e}"))?;
        }
        let key = PKey::private_key_from_der(self.key.as_slice()).unwrap();
        ssl_builder
            .set_private_key(&key)
            .map_err(|e| anyhow!("failed to set private key: {e}"))?;
        Ok(())
    }
}
