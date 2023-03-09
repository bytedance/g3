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

use anyhow::anyhow;
use rcgen::{Certificate, CertificateParams, DnType, KeyPair};

use crate::frontend::ResponseData;

pub(crate) struct RcGenBackendConfig {
    ca_cert: Arc<Certificate>,
}

pub(crate) struct RcGenBackend {
    ca_cert: Arc<Certificate>,
}

impl RcGenBackendConfig {
    pub(crate) fn new(ca_cert: &str, ca_key: &str) -> anyhow::Result<Self> {
        let ca_key =
            KeyPair::from_pem(ca_key).map_err(|e| anyhow!("failed to parse ca_key: {e}"))?;
        let params = CertificateParams::from_ca_cert_pem(ca_cert, ca_key)
            .map_err(|e| anyhow!("failed to parse ca_cert: {e}"))?;
        let ca_cert = Certificate::from_params(params)
            .map_err(|e| anyhow!("invalid ca_cert / ca_key pair: {e}"))?;
        Ok(RcGenBackendConfig {
            ca_cert: Arc::new(ca_cert),
        })
    }
}

impl RcGenBackend {
    pub(crate) fn new(config: &RcGenBackendConfig) -> Self {
        RcGenBackend {
            ca_cert: Arc::clone(&config.ca_cert),
        }
    }

    pub(crate) fn generate(&self, host: &str) -> anyhow::Result<ResponseData> {
        let mut params = CertificateParams::new([host.to_string()]);
        params.alg = &rcgen::PKCS_ECDSA_P256_SHA256;
        params.distinguished_name.push(DnType::CommonName, host);
        let cert = Certificate::from_params(params)
            .map_err(|e| anyhow!("failed to generate new cert/key pair: {e}"))?;
        let cert_pem = cert.serialize_pem_with_signer(&self.ca_cert).unwrap();
        let key_pem = cert.serialize_private_key_pem();
        let data = ResponseData {
            host: host.to_string(),
            cert: cert_pem,
            key: key_pem,
            ttl: 300,
        };
        Ok(data)
    }
}
