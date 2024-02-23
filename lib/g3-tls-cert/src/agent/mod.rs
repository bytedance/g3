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
use openssl::ssl::SslRef;
use openssl::x509::X509;

mod query;
use query::QueryRuntime;

mod config;
pub use config::CertAgentConfig;

mod handle;
pub use handle::CertAgentHandle;

#[derive(Clone, Debug, Hash, Eq, PartialEq)]
pub(crate) struct CacheQueryKey {
    pub(crate) host: String,
}

#[derive(Clone)]
pub struct FakeCertPair {
    certs: Vec<X509>,
    key: PKey<Private>,
}

impl FakeCertPair {
    pub fn add_to_ssl(self, ssl: &mut SslRef) -> anyhow::Result<()> {
        let FakeCertPair { certs, key } = self;
        let mut certs_iter = certs.into_iter();
        let Some(leaf_cert) = certs_iter.next() else {
            return Err(anyhow!("no certificate found"));
        };
        ssl.set_certificate(&leaf_cert)
            .map_err(|e| anyhow!("failed to set certificate: {e}"))?;
        for cert in certs_iter {
            ssl.add_chain_cert(cert)
                .map_err(|e| anyhow!("failed to add chain cert: {e}"))?;
        }
        ssl.set_private_key(&key)
            .map_err(|e| anyhow!("failed to set private key: {e}"))?;
        Ok(())
    }
}
