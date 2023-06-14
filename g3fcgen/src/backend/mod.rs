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

use std::str::FromStr;
use std::sync::Arc;

use anyhow::anyhow;
use openssl::pkey::{PKey, Private};
use openssl::x509::X509;

use g3_tls_cert::builder::ServerCertBuilder;
use g3_types::net::Host;

use crate::frontend::ResponseData;

pub(crate) struct OpensslBackendConfig {
    ca_cert: X509,
    ca_key: PKey<Private>,
}

impl OpensslBackendConfig {
    pub(crate) fn new(ca_cert: &str, ca_key: &str) -> anyhow::Result<Self> {
        let ca_key = PKey::private_key_from_pem(ca_key.as_bytes())
            .map_err(|e| anyhow!("failed to load ca pkey: {e}"))?;
        let ca_cert = X509::from_pem(ca_cert.as_bytes())
            .map_err(|e| anyhow!("failed to load ca cert: {e}"))?;

        Ok(OpensslBackendConfig { ca_cert, ca_key })
    }
}

pub(crate) struct OpensslBackend {
    config: Arc<OpensslBackendConfig>,
    builder: ServerCertBuilder,
}

impl OpensslBackend {
    pub(crate) fn new(config: &Arc<OpensslBackendConfig>) -> anyhow::Result<Self> {
        let builder = ServerCertBuilder::new_ec256()?;
        Ok(OpensslBackend {
            config: Arc::clone(config),
            builder,
        })
    }

    pub(crate) fn refresh(&mut self) -> anyhow::Result<()> {
        self.builder.refresh_datetime()?;
        self.builder.refresh_pkey()?;
        self.builder.refresh_serial()?;
        Ok(())
    }

    pub(crate) fn generate(&self, host: &str) -> anyhow::Result<ResponseData> {
        let host = Host::from_str(host)?;
        let cert =
            self.builder
                .build_fake(&host, &self.config.ca_cert, &self.config.ca_key, None)?;
        let cert_pem = cert
            .to_pem()
            .map_err(|e| anyhow!("failed to encode cert: {e}"))?;
        let key_pem = self
            .builder
            .pkey()
            .private_key_to_pem_pkcs8()
            .map_err(|e| anyhow!("failed to encode pkey: {e}"))?;

        let data = ResponseData {
            host: host.to_string(),
            cert: unsafe { String::from_utf8_unchecked(cert_pem) },
            key: unsafe { String::from_utf8_unchecked(key_pem) },
            ttl: 300,
        };
        Ok(data)
    }
}
